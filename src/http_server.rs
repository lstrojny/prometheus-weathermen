use crate::config;
use config::NAME;
use log::{debug, error, info, trace};
use rocket::http::{Accept, ContentType, Header, MediaType, QMediaType, Status};
use rocket::{get, Either, Responder, State};
use rocket_basicauth::BasicAuth;
use std::cmp::Ordering;

use crate::config::CredentialsStore;
use crate::config::ProviderTasks;

use crate::prometheus::{format_metrics, Format};
use crate::providers::Weather;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use tokio::task::JoinError;

#[get("/")]
#[allow(clippy::needless_pass_by_value)]
pub fn index(
    credentials_store: &State<Option<CredentialsStore>>,
    credentials_presented: Option<BasicAuth>,
    accept: &Accept,
) -> Result<MetricsResponse, Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials_store, &credentials_presented) {
        Ok(_) => Ok(MetricsResponse::new(
            Status::NotFound,
            get_metrics_format(accept),
            "Check /metrics".into(),
        )),
        Err(e) => auth_error_to_response(&e),
    }
}

#[get("/metrics")]
pub async fn metrics(
    unscheduled_tasks: &State<ProviderTasks>,
    credentials_store: &State<Option<CredentialsStore>>,
    credentials_presented: Option<BasicAuth>,
    accept: &Accept,
) -> Result<MetricsResponse, Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials_store, &credentials_presented) {
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
        Self {
            content_type: if status.class().is_success() {
                match content_type {
                    Format::OpenMetrics => Some(
                        ContentType::new("application", "openmetrics-text")
                            .with_params([("version", "1.0.0"), ("charset", "utf-8")]),
                    ),
                    Format::Prometheus => None,
                }
            } else {
                None
            }
            .unwrap_or_else(|| ContentType::new("text", "plain").with_params(("charset", "utf-8"))),
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

#[derive(Debug, PartialEq, Eq)]
pub enum Granted {
    NotRequired,
    Succeeded,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Denied {
    Unauthorized,
    Forbidden,
}

pub fn maybe_authenticate(
    credentials_store: &Option<CredentialsStore>,
    credentials_presented: &Option<BasicAuth>,
) -> Result<Granted, Denied> {
    match (credentials_store, credentials_presented) {
        (Some(credentials_store), Some(credentials_presented)) => {
            authenticate(credentials_store, credentials_presented)
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

fn authenticate(credentials: &CredentialsStore, auth: &BasicAuth) -> Result<Granted, Denied> {
    for (username, hash) in credentials.0.clone() {
        if username != auth.username {
            continue;
        }

        return match bcrypt::verify(auth.password.as_bytes(), &hash) {
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
        };
    }

    Err(Denied::Forbidden)
}

fn sort_media_types(accept: &Accept) -> Vec<&QMediaType> {
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

fn get_metrics_format(accept: &Accept) -> Format {
    let open_metrics_media_type = MediaType::new("application", "openmetrics-text");
    let text_plain_media_type = MediaType::new("text", "plain");

    let text_plain_q_media_type: QMediaType = text_plain_media_type.clone().into();

    let vec = sort_media_types(accept);

    let media_type = vec
        .iter()
        .find(|&media_type| {
            media_type.media_type() == &open_metrics_media_type
                || media_type.media_type() == &text_plain_media_type
        })
        .unwrap_or(&&text_plain_q_media_type)
        .media_type();

    if media_type == &open_metrics_media_type {
        Format::OpenMetrics
    } else {
        Format::Prometheus
    }
}

#[cfg(test)]
mod tests {
    mod authentication {
        use crate::config::CredentialsStore;
        use crate::http_server::{maybe_authenticate, Denied, Granted};
        use rocket_basicauth::BasicAuth;

        #[test]
        fn false_if_no_authentication_required() {
            assert_eq!(Ok(Granted::NotRequired), maybe_authenticate(&None, &None))
        }

        #[test]
        fn unauthorized_if_no_auth_information_provided() {
            assert_eq!(
                Err(Denied::Unauthorized),
                maybe_authenticate(&Some(CredentialsStore::empty()), &None)
            );
        }

        #[test]
        fn forbidden_if_username_not_found() {
            assert_eq!(
                Err(Denied::Forbidden),
                maybe_authenticate(
                    &Some(CredentialsStore::empty()),
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
                    &Some(CredentialsStore::from([(
                        "joanna".into(),
                        "$2a$12$KR9glOH.QnpZ8TTZzkRFfO2GejbHoPFyBtViBgPWND764MQy735Q6".into()
                    )])),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "incorrect".into()
                    })
                )
            );
        }

        #[test]
        fn granted_if_authentication_successful() {
            assert_eq!(
                Ok(Granted::Succeeded),
                maybe_authenticate(
                    &Some(CredentialsStore::from([(
                        "joanna".into(),
                        "$2a$04$58bTU55Vh8w9N5NX/DCCT.FY7ugMX06E1fFK.vtVVxOUdJYrAUlna".into()
                    )])),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "secret".into()
                    })
                )
            );
        }
    }

    mod content_negotiation {
        use crate::http_server::{get_metrics_format, sort_media_types};
        use crate::prometheus::Format;
        use rocket::http::{Accept, MediaType, QMediaType};
        use std::str::FromStr;

        #[test]
        fn sort_prefer_media_type_without_priority() {
            assert_eq!(
                vec![
                    &QMediaType(MediaType::new("text", "html"), None),
                    &QMediaType(MediaType::new("application", "json"), None),
                    &QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                    &QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                ],
                sort_media_types(&Accept::new(vec![
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
                sort_media_types(&Accept::new(vec![
                    QMediaType(MediaType::new("text", "plain"), None),
                    QMediaType(MediaType::new("application", "json"), Some(0.1)),
                    QMediaType(MediaType::new("application", "openmetrics-text"), Some(0.9)),
                ]))
            );
        }

        #[test]
        fn sort_prefer_media_type_without_priority_from_string() {
            assert_eq!(
                vec!["text/plain", "application/openmetrics-text"],
                sort_media_types(
                    &Accept::from_str("text/plain, application/openmetrics-text;q=0.9")
                        .expect("Must parse")
                )
                .iter()
                .map(|v| format!("{}/{}", v.media_type().top(), v.media_type().sub()))
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
                sort_media_types(&Accept::new(vec![
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
                sort_media_types(&Accept::new(vec![
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
                        Some(0.9)
                    ),
                    &QMediaType(
                        MediaType::new("application", "json")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9)
                    ),
                    &QMediaType(
                        MediaType::new("text", "plain").with_params(vec![("charset", "utf8")]),
                        Some(0.9)
                    ),
                    &QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                ],
                sort_media_types(&Accept::new(vec![
                    QMediaType(MediaType::new("text", "plain"), Some(0.9)),
                    QMediaType(
                        MediaType::new("text", "plain").with_params(vec![("charset", "utf8")]),
                        Some(0.9)
                    ),
                    QMediaType(
                        MediaType::new("text", "plain")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9)
                    ),
                    QMediaType(
                        MediaType::new("application", "json")
                            .with_params(vec![("charset", "utf8"), ("version", "0.1")]),
                        Some(0.9)
                    ),
                ]))
            );
        }

        #[test]
        fn prometheus_if_no_preference() {
            assert_eq!(Format::Prometheus, get_metrics_format(&Accept::new(vec![])))
        }

        #[test]
        fn open_metrics_if_available() {
            assert_eq!(
                Format::OpenMetrics,
                get_metrics_format(&Accept::new(vec![MediaType::new(
                    "application",
                    "openmetrics-text"
                )
                .into()]))
            )
        }

        #[test]
        fn text_plain_if_only_available() {
            assert_eq!(
                Format::Prometheus,
                get_metrics_format(&Accept::new(vec![MediaType::new("text", "plain").into()]))
            )
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
                        Some(0.9)
                    ),
                ]))
            );
        }
    }
}
