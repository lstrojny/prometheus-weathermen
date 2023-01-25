use crate::config;
use config::NAME;
use log::{debug, error, info, trace};
use rocket::http::{Header, Status};
use rocket::{get, Either, Responder, State};
use rocket_basicauth::BasicAuth;

use crate::config::CredentialsStore;
use crate::config::ProviderTasks;

use crate::prometheus::format;
use crate::providers::Weather;
use rocket::tokio::task;
use rocket::tokio::task::JoinSet;
use tokio::task::JoinError;

#[get("/")]
#[allow(clippy::needless_pass_by_value)]
pub fn index(
    credentials_store: &State<Option<CredentialsStore>>,
    credentials_presented: Option<BasicAuth>,
) -> Result<(Status, &'static str), Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials_store, &credentials_presented) {
        Ok(_) => Ok((Status::NotFound, "Check /metrics")),
        Err(e) => auth_error_to_response(&e),
    }
}

#[get("/metrics")]
pub async fn metrics(
    unscheduled_tasks: &State<ProviderTasks>,
    credentials_store: &State<Option<CredentialsStore>>,
    credentials_presented: Option<BasicAuth>,
) -> Result<(Status, String), Either<UnauthorizedResponse, ForbiddenResponse>> {
    match maybe_authenticate(credentials_store, &credentials_presented) {
        Ok(_) => Ok(serve_metrics(unscheduled_tasks).await),
        Err(e) => auth_error_to_response(&e),
    }
}

async fn serve_metrics(unscheduled_tasks: &State<ProviderTasks>) -> (Status, String) {
    let mut join_set = JoinSet::new();

    #[allow(clippy::unnecessary_to_owned)]
    for task in unscheduled_tasks.to_vec() {
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

    wait_for_metrics(join_set).await.map_or_else(
        |e| {
            error!("General error while fetching weather data: {e}");
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
        result??.map_or_else(
            |e| error!("Provider error while fetching weather data: {e}"),
            |w| weather.push(w),
        );
    }

    format(weather)
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
        (None, _) => {
            trace!("No credentials store configured, authentication required");
            Ok(Granted::NotRequired)
        }
        (_, None) => {
            trace!("No credentials presented. Unauthorized");
            Err(Denied::Unauthorized)
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

#[cfg(test)]
mod tests {
    use crate::config::CredentialsStore;
    use crate::http::{maybe_authenticate, Denied, Granted};
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
    fn unit_if_authentication_successful() {
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
