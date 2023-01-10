use crate::config;
use config::NAME;
use log::{debug, error, info, trace};
use rocket::http::{Header, Status};
use rocket::{get, Either, Responder, State};
use rocket_basicauth::BasicAuth;

use crate::config::Credentials;
use crate::config::ProviderTasks;

use crate::prometheus::format;
use crate::providers::Weather;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use tokio::task::JoinError;

#[get("/")]
#[allow(clippy::needless_pass_by_value)]
pub fn index(
    credentials: &State<Option<Credentials>>,
    auth: Option<BasicAuth>,
) -> Result<(Status, &'static str), Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials, &auth) {
        Ok(_) => Ok((Status::NotFound, "Check /metrics")),
        Err(e) => Err(e),
    }
}

#[get("/metrics")]
pub async fn metrics(
    unscheduled_tasks: &State<ProviderTasks>,
    credentials: &State<Option<Credentials>>,
    auth: Option<BasicAuth>,
) -> Result<(Status, String), Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials, &auth) {
        Ok(_) => Ok(serve_metrics(unscheduled_tasks).await),
        Err(e) => Err(e),
    }
}

async fn serve_metrics(unscheduled_tasks: &State<ProviderTasks>) -> (Status, String) {
    let mut join_set = JoinSet::new();

    #[allow(clippy::unnecessary_to_owned)]
    for (provider, req, cache) in unscheduled_tasks.to_vec() {
        let prov_req = req.clone();
        let task_cache = cache.clone();
        join_set.spawn(task::spawn_blocking(move || {
            info!(
                "Requesting weather data for {:?} from {:?} ({:?})",
                prov_req.name,
                provider.id(),
                prov_req.query,
            );
            provider.for_coordinates(&task_cache, &prov_req)
        }));
    }

    wait_for_metrics(join_set).await.map_or_else(
        |e| {
            error!("Error while fetching weather data: {e}");
            (
                Status::InternalServerError,
                "Error while fetching weather data. Check the logs".into(),
            )
        },
        |metrics| (Status::Ok, metrics),
    )
}

async fn wait_for_metrics(
    mut join_set: JoinSet<Result<anyhow::Result<Weather>, JoinError>>,
) -> anyhow::Result<String> {
    let mut weather = vec![];

    while let Some(result) = join_set.join_next().await {
        weather.push(result???);
    }

    format(weather)
}

#[derive(Responder, Debug, PartialEq, Eq)]
#[response(content_type = "text/plain")]
pub struct UnauthorizedResponse {
    response: (Status, &'static str),
    authenticate: Header<'static>,
}

impl UnauthorizedResponse {
    fn new() -> Self {
        Self {
            response: (
                Status::Unauthorized,
                "Authentication required. No credentials provided",
            ),
            authenticate: Header::new(
                "www-authenticate",
                format!(r##"Basic realm="{NAME}", charset="UTF-8""##),
            ),
        }
    }
}

#[derive(Responder, Debug, PartialEq, Eq)]
#[response(content_type = "text/plain")]
pub struct ForbiddenResponse {
    response: (Status, &'static str),
}

impl ForbiddenResponse {
    const fn new() -> Self {
        Self {
            response: (Status::Forbidden, "Access denied. Invalid credentials"),
        }
    }
}

pub fn maybe_authenticate(
    credentials: &Option<Credentials>,
    auth: &Option<BasicAuth>,
) -> Result<bool, Either<UnauthorizedResponse, ForbiddenResponse>> {
    if credentials.is_none() {
        trace!("No authentication required");
        return Ok(false);
    }

    if auth.is_none() {
        return Err(Either::Left(UnauthorizedResponse::new()));
    }

    authenticate(credentials.as_ref().unwrap(), auth.as_ref().unwrap())
}

fn authenticate(
    credentials: &Credentials,
    auth: &BasicAuth,
) -> Result<bool, Either<UnauthorizedResponse, ForbiddenResponse>> {
    for (username, hash) in credentials.0.clone() {
        if username != auth.username {
            continue;
        }

        return match bcrypt::verify(auth.password.as_bytes(), &hash) {
            Ok(r) => {
                if r {
                    debug!("Username {username:?} successfully authenticated");
                    Ok(true)
                } else {
                    debug!("Invalid password for {username:?}");
                    Err(Either::Right(ForbiddenResponse::new()))
                }
            }
            Err(e) => {
                error!("Error verifying bcrypt hash for {username:?}: {e:?}");
                Err(Either::Right(ForbiddenResponse::new()))
            }
        };
    }

    Err(Either::Right(ForbiddenResponse::new()))
}

#[cfg(test)]
mod tests {
    use crate::config::Credentials;
    use crate::http::{maybe_authenticate, ForbiddenResponse, UnauthorizedResponse};
    use rocket_basicauth::BasicAuth;

    #[test]
    fn false_if_no_authentication_required() {
        assert_eq!(
            false,
            maybe_authenticate(&None, &None).expect("Expect result")
        )
    }

    #[test]
    fn unauthorized_if_no_auth_information_provided() {
        assert_eq!(
            UnauthorizedResponse::new(),
            maybe_authenticate(&Some(Credentials::empty()), &None)
                .expect_err("Error expected")
                .expect_left("Unauthorized response expected")
        );
    }

    #[test]
    fn forbidden_if_username_not_found() {
        assert_eq!(
            ForbiddenResponse::new(),
            maybe_authenticate(
                &Some(Credentials::empty()),
                &Some(BasicAuth {
                    username: "joanna".into(),
                    password: "secret".into()
                })
            )
            .expect_err("Error expected")
            .expect_right("Forbidden response expected")
        );
    }

    #[test]
    fn forbidden_if_incorrect_password() {
        assert_eq!(
            ForbiddenResponse::new(),
            maybe_authenticate(
                &Some(Credentials::from([(
                    "joanna".into(),
                    "$2a$12$KR9glOH.QnpZ8TTZzkRFfO2GejbHoPFyBtViBgPWND764MQy735Q6".into()
                )])),
                &Some(BasicAuth {
                    username: "joanna".into(),
                    password: "incorrect".into()
                })
            )
            .expect_err("Error expected")
            .expect_right("Forbidden response expected")
        );
    }

    #[test]
    fn unit_if_authentication_successful() {
        assert_eq!(
            true,
            maybe_authenticate(
                &Some(Credentials::from([(
                    "joanna".into(),
                    "$2a$04$58bTU55Vh8w9N5NX/DCCT.FY7ugMX06E1fFK.vtVVxOUdJYrAUlna".into()
                )])),
                &Some(BasicAuth {
                    username: "joanna".into(),
                    password: "secret".into()
                })
            )
            .expect("Expect result")
        );
    }
}
