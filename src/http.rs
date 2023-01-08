use crate::config;
use crate::config::Credentials;
use config::NAME;
use log::{debug, error, trace};
use rocket::http::{Header, Status};
use rocket::{Either, Responder};
use rocket_basicauth::BasicAuth;

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
                    username: "joanna".to_string(),
                    password: "secret".to_string()
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
                    "joanna".to_string(),
                    "$2a$12$KR9glOH.QnpZ8TTZzkRFfO2GejbHoPFyBtViBgPWND764MQy735Q6".to_string()
                )])),
                &Some(BasicAuth {
                    username: "joanna".to_string(),
                    password: "incorrect".to_string()
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
                    "joanna".to_string(),
                    "$2a$04$58bTU55Vh8w9N5NX/DCCT.FY7ugMX06E1fFK.vtVVxOUdJYrAUlna".to_string()
                )])),
                &Some(BasicAuth {
                    username: "joanna".to_string(),
                    password: "secret".to_string()
                })
            )
            .expect("Expect result")
        );
    }
}
