use crate::config::NAME;
use log::{debug, error, info, trace};
use moka::sync::{Cache, CacheBuilder};
use once_cell::sync::Lazy;
use rocket::http::{Accept, ContentType, Header, MediaType, QMediaType, Status};
use rocket::{get, routes, Build, Either, Responder, Rocket, State};
use rocket_basicauth::BasicAuth;
use std::cmp::Ordering;

use crate::config::ProviderTasks;
use crate::config::{get_provider_tasks, Config, CredentialsStore};

use crate::bcrypt::{get_cost, get_default_hash, Hash};
use crate::error::exit_if_handle_fatal;
use crate::prometheus::{format_metrics, Format};
use crate::providers::Weather;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use tokio::task::JoinError;

pub async fn configure_rocket(config: Config) -> Rocket<Build> {
    let config_clone = config.clone();
    let tasks = task::spawn_blocking(move || get_provider_tasks(config_clone))
        .await
        .unwrap_or_else(exit_if_handle_fatal)
        .unwrap_or_else(exit_if_handle_fatal);

    let configured_credentials = config.auth.and_then(|store| {
        get_default_hash(get_max_costs_from_credentials_store(&store))
            .map(|default_hash| (store, default_hash))
    });

    #[allow(clippy::no_effect_underscore_binding)]
    rocket::custom(config.http)
        .manage(tasks)
        .manage(configured_credentials)
        .mount("/", routes![index, metrics])
}

fn get_max_costs_from_credentials_store(credentials_store: &CredentialsStore) -> Option<u32> {
    credentials_store
        .clone()
        .into_iter()
        .map(|(_, hash)| get_cost(&hash))
        .max()
        .flatten()
}

#[get("/")]
#[allow(clippy::needless_pass_by_value)]
fn index(
    credentials_configured: &State<Option<(CredentialsStore, Hash)>>,
    credentials_presented: Option<BasicAuth>,
    accept: &Accept,
) -> Result<MetricsResponse, Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials_configured, &credentials_presented) {
        Ok(_) => Ok(MetricsResponse::new(
            Status::NotFound,
            get_metrics_format(accept),
            "Check /metrics".into(),
        )),
        Err(e) => auth_error_to_response(&e),
    }
}

#[get("/metrics")]
async fn metrics(
    unscheduled_tasks: &State<ProviderTasks>,
    credentials_configured: &State<Option<(CredentialsStore, Hash)>>,
    credentials_presented: Option<BasicAuth>,
    accept: &Accept,
) -> Result<MetricsResponse, Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials_configured, &credentials_presented) {
        Ok(_) => Ok(serve_metrics(get_metrics_format(accept), unscheduled_tasks).await),
        Err(e) => auth_error_to_response(&e),
    }
}

async fn serve_metrics(
    format: Format,
    unscheduled_tasks: &State<ProviderTasks>,
) -> MetricsResponse {
    let mut join_set = JoinSet::new();

    for task in unscheduled_tasks.iter().cloned() {
        join_set.spawn(task::spawn_blocking(move || {
            info!(
                "Requesting weather data for {:?} from {:?} ({:?})",
                task.request.name,
                task.provider.id(),
                task.request.query,
            );
            task.provider
                .for_coordinates(&task.client, &task.cache, &task.request)
        }));
    }

    wait_for_metrics(format, join_set).await.map_or_else(
        |e| {
            error!("General error while fetching weather data: {e}");
            MetricsResponse::new(
                Status::InternalServerError,
                format,
                "Error while fetching weather data. Check the logs".into(),
            )
        },
        |metrics| MetricsResponse::new(Status::Ok, format, metrics),
    )
}

async fn wait_for_metrics(
    format: Format,
    mut join_set: JoinSet<Result<anyhow::Result<Weather>, JoinError>>,
) -> anyhow::Result<String> {
    let mut weather = vec![];

    while let Some(result) = join_set.join_next().await {
        result??.map_or_else(
            |e| error!("Provider error while fetching weather data: {e}"),
            |w| weather.push(w),
        );
    }

    format_metrics(format, weather)
}

fn auth_error_to_response<T>(
    error: &Denied,
) -> Result<T, Either<UnauthorizedResponse, ForbiddenResponse>> {
    match error {
        Denied::Unauthorized => Err(Either::Left(UnauthorizedResponse::new())),
        Denied::Forbidden => Err(Either::Right(ForbiddenResponse::new())),
    }
}

#[derive(Responder, Debug, PartialEq, Eq)]
#[response()]
pub struct MetricsResponse {
    response: (Status, String),
    content_type: ContentType,
}

impl MetricsResponse {
    fn new(status: Status, content_type: Format, response: String) -> Self {
        let content_type = if status.class().is_success() && content_type == Format::OpenMetrics {
            get_openmetrics_content_type()
        } else {
            get_text_plain_content_type()
        };

        Self {
            content_type,
            response: (status, response),
        }
    }
}

#[derive(Responder, Debug, PartialEq, Eq)]
#[response(content_type = "text/plain; charset=utf-8", status = 401)]
pub struct UnauthorizedResponse {
    response: &'static str,
    authenticate: Header<'static>,
}

impl UnauthorizedResponse {
    fn new() -> Self {
        Self {
            response: "Authentication required. No credentials provided",
            authenticate: Header::new(
                "www-authenticate",
                format!(r##"Basic realm="{NAME}", charset="UTF-8""##),
            ),
        }
    }
}

#[derive(Responder, Debug, PartialEq, Eq)]
#[response(content_type = "text/plain; charset=utf-8", status = 403)]
pub struct ForbiddenResponse {
    response: &'static str,
}

impl ForbiddenResponse {
    const fn new() -> Self {
        Self {
            response: "Access denied. Invalid credentials",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Granted {
    NotRequired,
    Succeeded,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Denied {
    Unauthorized,
    Forbidden,
}

pub fn maybe_authenticate(
    credentials_configured: &Option<(CredentialsStore, Hash)>,
    credentials_presented: &Option<BasicAuth>,
) -> Result<Granted, Denied> {
    match (credentials_configured, credentials_presented) {
        (Some((credentials_store, default_hash)), Some(credentials_presented)) => {
            authenticate(credentials_store, credentials_presented, default_hash)
        }
        (Some(_), None) => {
            trace!("No credentials presented. Unauthorized");
            Err(Denied::Unauthorized)
        }
        (None, _) => {
            trace!("No credentials store configured, skipping authentication");
            Ok(Granted::NotRequired)
        }
    }
}

static AUTHENTICATION_CACHE: Lazy<Cache<(String, String), Result<Granted, Denied>>> =
    Lazy::new(|| CacheBuilder::new(10u64.pow(6)).build());

fn authenticate(
    credentials: &CredentialsStore,
    auth: &BasicAuth,
    default_hash: &Hash,
) -> Result<Granted, Denied> {
    for (username, hash) in credentials.clone() {
        if username != auth.username {
            continue;
        }

        return AUTHENTICATION_CACHE
            .entry((auth.username.clone(), auth.password.clone()))
            .or_insert_with_if(
                || match bcrypt::verify(auth.password.as_bytes(), &hash) {
                    Ok(r) => {
                        if r {
                            debug!("Username {username:?} successfully authenticated");
                            Ok(Granted::Succeeded)
                        } else {
                            debug!("Invalid password for {username:?}");
                            Err(Denied::Forbidden)
                        }
                    }
                    Err(e) => {
                        error!("Error verifying bcrypt hash for {username:?}: {e:?}");
                        Err(Denied::Forbidden)
                    }
                },
                Result::is_err,
            )
            .into_value();
    }

    let default_hash_string: String = default_hash.to_string();
    // Prevent timing attacks by making sure one bcrypt operation is run each time
    let _ignored = bcrypt::verify(auth.password.as_bytes(), default_hash_string.as_str());

    Err(Denied::Forbidden)
}

fn sort_media_types_by_priority(accept: &Accept) -> Vec<&QMediaType> {
    let mut vec: Vec<&QMediaType> = accept.iter().collect();
    vec.sort_by(|&left, &right| {
        right
            .weight()
            .map_or(Ordering::Greater, |right_weight| {
                // Absence of weight parameter means most important
                left.weight().map_or(Ordering::Less, |left_weight| {
                    // The higher the weight, the higher the priority
                    right_weight
                        .partial_cmp(&left_weight)
                        .unwrap_or(Ordering::Equal)
                })
            })
            // The more specific, the higher the priority
            .then_with(|| right.specificity().cmp(&left.specificity()))
            // The more parameters, the higher the priority
            .then_with(|| right.params().count().cmp(&left.params().count()))
    });

    trace!("Sorted list of accepted media types: {:#?}", vec);

    vec
}

const fn get_content_type_params(version: &str) -> [(&str, &str); 2] {
    [("charset", "utf-8"), ("version", version)]
}

fn get_openmetrics_content_type() -> ContentType {
    ContentType::new("application", "openmetrics-text")
        .with_params(get_content_type_params("1.0.0"))
}

fn get_text_plain_content_type() -> ContentType {
    ContentType::new("text", "plain").with_params(get_content_type_params("0.0.4"))
}

static MEDIA_TYPE_FORMATS: Lazy<Vec<(MediaType, Format)>> = Lazy::new(|| {
    vec![
        (
            get_openmetrics_content_type().media_type().clone(),
            Format::OpenMetrics,
        ),
        (
            get_text_plain_content_type().media_type().clone(),
            Format::Prometheus,
        ),
    ]
});

fn get_metrics_format(accept: &Accept) -> Format {
    let media_types_by_priority = sort_media_types_by_priority(accept);

    media_types_by_priority
        .iter()
        .find_map(|&given_media_type| {
            MEDIA_TYPE_FORMATS
                .iter()
                .find_map(|(expected_media_type, format)| {
                    media_type_matches(expected_media_type, given_media_type.media_type())
                        .then_some(*format)
                })
        })
        .unwrap_or(Format::Prometheus)
}

fn media_type_matches(left: &MediaType, right: &MediaType) -> bool {
    left == right || (left.top() == right.top() && (left.sub() == "*" || right.sub() == "*"))
}

#[cfg(test)]
mod tests {
    mod authentication {
        use crate::config::CredentialsStore;
        use crate::http_server::{maybe_authenticate, Denied, Granted};
        use pretty_assertions::assert_eq;
        use rocket_basicauth::BasicAuth;

        const DEFAULT_HASH: &str = "$2y$04$/iGPpt/.lDZV7ezOrIpV5ubAd5DF9d.6TCh/ClYqZ/JXXqcCISEfW";
        const SECRET_HASH: &str = "$2y$04$RLR0zzNVe3K8eJg/NaRUxuWvIEXys0BwG0SnopFZ0K12Xei7HGq2i";

        #[test]
        fn false_if_no_authentication_required() {
            assert_eq!(Ok(Granted::NotRequired), maybe_authenticate(&None, &None));
        }

        #[test]
        fn unauthorized_if_no_auth_information_provided() {
            assert_eq!(
                Err(Denied::Unauthorized),
                maybe_authenticate(
                    &Some((CredentialsStore::default(), DEFAULT_HASH.to_string().into())),
                    &None
                )
            );
        }

        #[test]
        fn forbidden_if_username_not_found() {
            assert_eq!(
                Err(Denied::Forbidden),
                maybe_authenticate(
                    &Some((CredentialsStore::default(), DEFAULT_HASH.to_string().into())),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "secret".into()
                    })
                )
            );
        }

        #[test]
        fn forbidden_if_incorrect_password() {
            assert_eq!(
                Err(Denied::Forbidden),
                maybe_authenticate(
                    &Some((
                        CredentialsStore::from([("joanna".into(), SECRET_HASH.into())]),
                        DEFAULT_HASH.to_string().into()
                    )),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "incorrect".into()
                    })
                )
            );
        }

        #[test]
        fn forbidden_even_if_fakepassword() {
            assert_eq!(
                Err(Denied::Forbidden),
                maybe_authenticate(
                    &Some((
                        CredentialsStore::from([("joanna".into(), SECRET_HASH.into())]),
                        DEFAULT_HASH.to_string().into()
                    )),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "fakepassword".into()
                    })
                )
            );
        }

        #[test]
        fn granted_if_authentication_successful() {
            assert_eq!(
                Ok(Granted::Succeeded),
                maybe_authenticate(
                    &Some((
                        CredentialsStore::from([("joanna".into(), SECRET_HASH.into())]),
                        DEFAULT_HASH.to_string().into()
                    )),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "secret".into(),
                    }),
                )
            );
        }

        #[cfg(feature = "nightly")]
        mod benchmark {
            extern crate test ;
            use crate::config::CredentialsStore;
            use crate::http_server::tests::authentication::{DEFAULT_HASH, SECRET_HASH};
            use crate::http_server::{authenticate, AUTHENTICATION_CACHE};
            use rocket_basicauth::BasicAuth;
            use test::Bencher;

            #[bench]
            fn bench_user_not_found(b: &mut Bencher) {
                b.iter(|| {
                    authenticate(
                        &CredentialsStore::default(),
                        &BasicAuth {
                            username: "joanna".into(),
                            password: "secret".into(),
                        },
                        &DEFAULT_HASH.to_string().into(),
                    )
                });
            }

            #[bench]
            fn bench_invalid_password(b: &mut Bencher) {
                b.iter(|| {
                    authenticate(
                        &CredentialsStore::from([("joanna".into(), SECRET_HASH.into())]),
                        &BasicAuth {
                            username: "joanna".into(),
                            password: "incorrect".into(),
                        },
                        &DEFAULT_HASH.to_string().into(),
                    )
                })
            }

            #[bench]
            fn bench_granted(b: &mut Bencher) {
                b.iter(|| {
                    AUTHENTICATION_CACHE.invalidate_all();
                    authenticate(
                        &CredentialsStore::from([("joanna".into(), SECRET_HASH.into())]),
                        &BasicAuth {
                            username: "joanna".into(),
                            password: "secret".into(),
                        },
                        &DEFAULT_HASH.to_string().into(),
                    )
                })
            }
        }
    }

    mod content_negotiation {
        use crate::http_server::{get_metrics_format, sort_media_types_by_priority};
        use crate::prometheus::Format;
        use pretty_assertions::assert_eq;
        use rocket::http::{Accept, MediaType, QMediaType};
        use std::str::FromStr;

        // See https://github.com/prometheus/prometheus/blob/75e5d600d9288cb1b573d6830356c94c991153a1/scrape/scrape.go#L785
        static PROMETHEUS_SCRAPER_ACCEPT_HEADER: &str = "application/openmetrics-text;version=1.0.0,application/openmetrics-text;version=0.0.1;q=0.75,text/plain;version=0.0.4;q=0.5,*/*;q=0.1";

        // See https://github.com/chromium/chromium/blob/04385e5d572e727897340223d54689e34ed6725e/content/common/content_constants_internal.cc#L24
        static CHROME_ACCEPT_HEADER: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7";

        // See https://searchfox.org/mozilla-central/rev/00ea1649b59d5f427979e2d6ba42be96f62d6e82/netwerk/protocol/http/nsHttpHandler.cpp#229
        static FIREFOX_ACCEPT_HEADER: &str =
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8";

        #[test]
        fn sort_prefer_media_type_without_priority() {
            assert_eq!(
                vec![
                    &QMediaType(MediaType::new("text", "html"), None),
                    &QMediaType(MediaType::new("application", "json"), None),
                    &QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    &QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                ],
                sort_media_types_by_priority(&Accept::new(vec![
                    QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    QMediaType(MediaType::new("text", "html"), None),
                    QMediaType(MediaType::new("application", "json"), None),
                    QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                ]))
            );

            assert_eq!(
                vec![
                    &QMediaType(MediaType::new("text", "plain"), None),
                    &QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    &QMediaType(MediaType::new("application", "json"), Some(0.1)),
                ],
                sort_media_types_by_priority(&Accept::new(vec![
                    QMediaType(MediaType::new("text", "plain"), None),
                    QMediaType(MediaType::new("application", "json"), Some(0.1)),
                    QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                ]))
            );
        }

        #[test]
        fn sort_prefer_media_type_without_priority_from_string() {
            assert_eq!(
                vec!["text/plain", "application/openmetrics-text; q=0.9"],
                sort_media_types_by_priority(
                    &Accept::from_str("text/plain, application/openmetrics-text;q=0.9")
                        .expect("Accept header value should parse")
                )
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>()
            );
        }

        #[test]
        fn sort_by_priority() {
            assert_eq!(
                vec![
                    &QMediaType(MediaType::new("text", "plain"), Some(1.0)),
                    &QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                ],
                sort_media_types_by_priority(&Accept::new(vec![
                    QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    QMediaType(MediaType::new("text", "plain"), Some(1.0)),
                ]))
            );
        }

        #[test]
        fn sort_by_specificity() {
            assert_eq!(
                vec![
                    &QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                    &QMediaType(MediaType::new("application", "*"), Some(0.9)),
                ],
                sort_media_types_by_priority(&Accept::new(vec![
                    QMediaType(MediaType::new("application", "*"), Some(0.9)),
                    QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                ]))
            );
        }

        #[test]
        fn sort_by_count_of_elements() {
            assert_eq!(
                vec![
                    &QMediaType(
                        MediaType::new("text", "plain")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9),
                    ),
                    &QMediaType(
                        MediaType::new("application", "json")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9),
                    ),
                    &QMediaType(
                        MediaType::new("text", "plain").with_params(vec![("charset", "utf8")]),
                        Some(0.9),
                    ),
                    &QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                ],
                sort_media_types_by_priority(&Accept::new(vec![
                    QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                    QMediaType(
                        MediaType::new("text", "plain").with_params(vec![("charset", "utf8")]),
                        Some(0.9),
                    ),
                    QMediaType(
                        MediaType::new("text", "plain")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9),
                    ),
                    QMediaType(
                        MediaType::new("application", "json")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9),
                    ),
                ]))
            );
        }

        #[test]
        fn sort_prometheus_header() {
            assert_eq!(
                vec![
                    "application/openmetrics-text; version=1.0.0",
                    "application/openmetrics-text; version=0.0.1; q=0.75",
                    "text/plain; version=0.0.4; q=0.5",
                    "*/*; q=0.1",
                ],
                sort_media_types_by_priority(
                    &Accept::from_str(PROMETHEUS_SCRAPER_ACCEPT_HEADER)
                        .expect("Accept header value should parse")
                )
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<String>>()
            );
        }

        #[test]
        fn sort_complicated_sorting() {
            assert_eq!(
                vec![
                    "text/plain; q=1.0; charset=utf-8; version=0.0.4",
                    "text/plain; q=0.95; version=0.0.4",
                    "application/openmetrics-text; q=0.9; version=1.0.0",
                    "application/openmetrics-text; q=0.8; version=0.0.1",
                    "*/*; q=0.1",
                ],
                sort_media_types_by_priority(
                    &Accept::from_str("application/openmetrics-text;q=0.9;version=1.0.0,application/openmetrics-text;q=0.8;version=0.0.1,text/plain;q=0.95;version=0.0.4,text/plain;q=1.0;charset=utf-8;version=0.0.4,*/*;q=0.1")
                        .expect("Must parse")
                )
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
            );
        }

        #[test]
        fn prometheus_if_no_preference() {
            assert_eq!(Format::Prometheus, get_metrics_format(&Accept::new(vec![])));
        }

        #[test]
        fn openmetrics_if_available() {
            assert_eq!(
                Format::OpenMetrics,
                get_metrics_format(&Accept::new(vec![MediaType::new(
                    "application",
                    "openmetrics-text",
                )
                .into()]))
            );
        }

        #[test]
        fn text_plain_if_only_available() {
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(&Accept::new(vec![MediaType::new("text", "plain").into()]))
            );
        }

        #[test]
        fn text_plain_if_higher_priority() {
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(&Accept::new(vec![
                    QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    QMediaType(MediaType::new("text", "plain"), Some(1.0)),
                ]))
            );
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(&Accept::new(vec![
                    QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    QMediaType(MediaType::new("text", "plain"), None),
                ]))
            );
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(&Accept::new(vec![
                    QMediaType(MediaType::new("application", "*"), Some(0.9)),
                    QMediaType(
                        MediaType::new("text", "plain").with_params(("charset", "utf-8")),
                        Some(0.9),
                    ),
                ]))
            );
        }

        #[test]
        fn openmetrics_if_more_specific() {
            assert_eq!(
                Format::OpenMetrics,
                get_metrics_format(
                    &Accept::from_str(
                        "text/*;q=0.95,application/openmetrics-text;q=0.95;version=1.0.0,*/*;q=0.1"
                    )
                    .expect("Must parse")
                )
            );
        }

        #[test]
        fn openmetrics_if_partial_match() {
            assert_eq!(
                Format::OpenMetrics,
                get_metrics_format(
                    &Accept::from_str("application/*,*/*;q=0.1")
                        .expect("Accept header value should parse")
                )
            );
            assert_eq!(
                Format::OpenMetrics,
                get_metrics_format(
                    &Accept::from_str("application/*;q=1.0,text/plain;q=0.9,*/*;q=0.1")
                        .expect("Must parse")
                )
            );
        }

        #[test]
        fn prometheus_for_firefox() {
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(
                    &Accept::from_str(FIREFOX_ACCEPT_HEADER)
                        .expect("Accept header value should parse")
                )
            );
        }

        #[test]
        fn prometheus_for_chrome() {
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(
                    &Accept::from_str(CHROME_ACCEPT_HEADER)
                        .expect("Accept header value should parse")
                )
            );
        }

        #[test]
        fn openmetrics_for_prometheus_scraper() {
            assert_eq!(
                Format::OpenMetrics,
                get_metrics_format(
                    &Accept::from_str(PROMETHEUS_SCRAPER_ACCEPT_HEADER)
                        .expect("Accept header value should parse")
                )
            );
        }
    }
}
