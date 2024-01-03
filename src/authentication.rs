use derive_more::{Display, From, Into};
use log::{debug, error, trace};
use moka::sync::{Cache, CacheBuilder};
use once_cell::sync::{Lazy, OnceCell};
use rocket::serde::{Deserialize, Serialize};
use rocket_basicauth::BasicAuth;
use std::collections::hash_map::Iter;
use std::collections::HashMap;

const BCRYPT_DEFAULT_PASSWORD: &str = "fakepassword";
const BCRYPT_DEFAULT_COST: u32 = bcrypt::DEFAULT_COST;

static BCRYPT_DEFAULT_HASH: OnceCell<Hash> = OnceCell::new();

static AUTHENTICATION_CACHE: Lazy<Cache<(String, String), Result<Granted, Denied>>> =
    Lazy::new(|| CacheBuilder::new(10_u64.pow(6)).build());

#[derive(Serialize, Deserialize, Debug, Into, Clone, Display, From)]
pub struct Hash(String);

impl Hash {
    fn cost(&self) -> Option<u32> {
        self.0.split('$').nth(2).and_then(|v| v.parse().ok())
    }
}

#[derive(Serialize, Deserialize, Debug, From, Clone, Default)]
pub struct CredentialsStore(HashMap<String, Hash>);

impl<const N: usize> From<[(String, Hash); N]> for CredentialsStore {
    fn from(arr: [(String, Hash); N]) -> Self {
        Self(HashMap::from(arr))
    }
}

impl CredentialsStore {
    pub(crate) fn iter(&self) -> Iter<String, Hash> {
        self.0.iter()
    }
    pub(crate) fn default_hash(&self) -> &Hash {
        BCRYPT_DEFAULT_HASH.get_or_init(|| Self::hash_default_password(self.max_cost()))
    }

    fn hash_default_password(cost: Option<u32>) -> Hash {
        bcrypt::hash(BCRYPT_DEFAULT_PASSWORD, cost.unwrap_or(BCRYPT_DEFAULT_COST))
            .ok()
            .map_or_else(
                || Self::hash_default_password(Some(BCRYPT_DEFAULT_COST)),
                Into::into,
            )
    }

    fn max_cost(&self) -> Option<u32> {
        self.0.values().map(Hash::cost).max().flatten()
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
    maybe_credentials_store: &Option<CredentialsStore>,
    maybe_credentials_presented: &Option<BasicAuth>,
) -> Result<Granted, Denied> {
    match (maybe_credentials_store, maybe_credentials_presented) {
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
    credentials
        .iter()
        .find_map(|(username, hash)| {
            (username == &auth.username).then(|| {
                AUTHENTICATION_CACHE
                    .entry((auth.username.clone(), auth.password.clone()))
                    .or_insert_with_if(
                        || match bcrypt::verify(auth.password.as_bytes(), &hash.to_string()) {
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
                    .into_value()
            })
        })
        .unwrap_or_else(|| {
            // Prevent timing attacks that could leak that a user does not exist.
            // If the user was not found above, make sure to run at least one bcrypt operation to keep the time constant.
            let _prevent_leak = bcrypt::verify(
                auth.password.as_bytes(),
                credentials.default_hash().to_string().as_str(),
            );
            Err(Denied::Forbidden)
        })
}

#[cfg(test)]
mod tests {
    mod default_hash {
        use crate::authentication::{CredentialsStore, Hash, BCRYPT_DEFAULT_COST};

        #[test]
        fn none_if_empty_string() {
            assert_eq!(Hash("".into()).cost(), None);
        }

        #[test]
        fn none_if_unparseable_string() {
            assert_eq!(Hash("$12".into()).cost(), None);
        }

        #[test]
        fn none_if_incomplete_string() {
            assert_eq!(Hash("$2a$".into()).cost(), None);
        }

        #[test]
        fn cost_128() {
            assert_eq!(
                Hash("$2a$255$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".into()).cost(),
                Some(255u32)
            );
        }

        #[test]
        fn cost_10() {
            assert_eq!(
                Hash("$2a$10$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".into()).cost(),
                Some(10u32)
            );
        }

        #[test]
        fn cost_5() {
            assert_eq!(
                Hash("$2a$05$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".into()).cost(),
                Some(5u32)
            );
        }

        #[test]
        fn cost_5_unpadded() {
            assert_eq!(
                Hash("$2a$5$R9h/cIPz0gi.URNNX3kh2OPST9/PgBkqquzi.Ss7KIUgO2t0jWMUW".into()).cost(),
                Some(5u32)
            );
        }

        #[test]
        fn default_hash_with_cost_too_low() {
            assert_default_hash_with_cost(Some(0), BCRYPT_DEFAULT_COST);
        }

        #[test]
        fn default_hash_with_cost_too_high() {
            assert_default_hash_with_cost(Some(255), BCRYPT_DEFAULT_COST);
        }

        #[test]
        fn default_hash_with_no_cost() {
            assert_default_hash_with_cost(None, BCRYPT_DEFAULT_COST);
        }

        #[test]
        fn default_hash_with_cost_ok() {
            assert_default_hash_with_cost(Some(5), 5);
        }

        fn assert_default_hash_with_cost(given_cost: Option<u32>, expected_cost: u32) {
            assert!(CredentialsStore::hash_default_password(given_cost)
                .to_string()
                .starts_with(format!("$2b${expected_cost:02}").as_str()));
        }
    }

    mod authentication {
        use crate::authentication::{maybe_authenticate, CredentialsStore, Denied, Granted};
        use pretty_assertions::assert_eq;
        use rocket_basicauth::BasicAuth;

        const SECRET_HASH: &str = "$2y$04$RLR0zzNVe3K8eJg/NaRUxuWvIEXys0BwG0SnopFZ0K12Xei7HGq2i";

        #[test]
        fn false_if_no_authentication_required() {
            assert_eq!(maybe_authenticate(&None, &None), Ok(Granted::NotRequired));
        }

        #[test]
        fn unauthorized_if_no_auth_information_provided() {
            assert_eq!(
                maybe_authenticate(&Some(CredentialsStore::default()), &None),
                Err(Denied::Unauthorized)
            );
        }

        #[test]
        fn forbidden_if_username_not_found() {
            assert_eq!(
                maybe_authenticate(
                    &Some(CredentialsStore::default()),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "secret".into()
                    })
                ),
                Err(Denied::Forbidden)
            );
        }

        #[test]
        fn forbidden_if_incorrect_password() {
            assert_eq!(
                maybe_authenticate(
                    &Some(CredentialsStore::from([(
                        "joanna".into(),
                        SECRET_HASH.to_string().into()
                    )])),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "incorrect".into()
                    })
                ),
                Err(Denied::Forbidden)
            );
        }

        #[test]
        fn forbidden_even_if_fakepassword() {
            assert_eq!(
                maybe_authenticate(
                    &Some(CredentialsStore::from([(
                        "joanna".to_string(),
                        SECRET_HASH.to_string().into()
                    )])),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "fakepassword".into()
                    })
                ),
                Err(Denied::Forbidden)
            );
        }

        #[test]
        fn granted_if_authentication_successful() {
            assert_eq!(
                maybe_authenticate(
                    &Some(CredentialsStore::from([(
                        "joanna".to_string(),
                        SECRET_HASH.to_string().into()
                    )])),
                    &Some(BasicAuth {
                        username: "joanna".into(),
                        password: "secret".into(),
                    }),
                ),
                Ok(Granted::Succeeded)
            );
        }

        #[cfg(feature = "nightly")]
        mod benchmark {
            extern crate test;
            use crate::authentication::tests::authentication::SECRET_HASH;
            use crate::authentication::{authenticate, CredentialsStore, AUTHENTICATION_CACHE};
            use rocket_basicauth::BasicAuth;
            use test::Bencher;

            fn credentials_store() -> CredentialsStore {
                CredentialsStore::from([("joanna".into(), SECRET_HASH.to_string().into())])
            }

            fn setup_benchmark_run() {
                credentials_store().default_hash();
            }

            fn setup_benchmark_iteration() {
                AUTHENTICATION_CACHE.invalidate_all();
            }

            #[bench]
            fn bench_user_not_found(b: &mut Bencher) {
                setup_benchmark_run();
                b.iter(|| {
                    setup_benchmark_iteration();
                    authenticate(
                        &credentials_store(),
                        &BasicAuth {
                            username: "unknown".into(),
                            password: "secret".into(),
                        },
                    )
                });
            }

            #[bench]
            fn bench_invalid_password(b: &mut Bencher) {
                setup_benchmark_run();
                b.iter(|| {
                    setup_benchmark_iteration();
                    authenticate(
                        &credentials_store(),
                        &BasicAuth {
                            username: "joanna".into(),
                            password: "incorrect".into(),
                        },
                    )
                })
            }

            #[bench]
            fn bench_granted(b: &mut Bencher) {
                setup_benchmark_run();
                b.iter(|| {
                    setup_benchmark_iteration();
                    authenticate(
                        &credentials_store(),
                        &BasicAuth {
                            username: "joanna".into(),
                            password: "secret".into(),
                        },
                    )
                })
            }
        }
    }
}
