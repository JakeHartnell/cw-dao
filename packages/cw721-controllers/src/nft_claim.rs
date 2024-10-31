use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, BlockInfo, CustomQuery, Deps, Order, StdError, StdResult, Storage};
use cw_storage_plus::{Bound, Map};
use cw_utils::Expiration;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum NftClaimError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("NFT claim not found for {token_id}")]
    NotFound { token_id: String },

    #[error("One or more NFTs is not ready to be claimed")]
    NotReady {},
}

#[cw_serde]
pub struct NftClaimsResponse {
    pub nft_claims: Vec<NftClaim>,
}

#[cw_serde]
pub struct NftClaim {
    pub token_id: String,
    pub release_at: Expiration,
}

impl NftClaim {
    pub fn new(token_id: String, release_at: Expiration) -> Self {
        NftClaim {
            token_id,
            release_at,
        }
    }
}

pub struct NftClaims<'a>(Map<'a, (&'a Addr, &'a String), Expiration>);

impl<'a> NftClaims<'a> {
    pub const fn new(storage_key: &'a str) -> Self {
        NftClaims(Map::new(storage_key))
    }

    /// Creates a number of NFT claims simultaneously for a given
    /// address.
    ///
    /// # Invariants
    ///
    /// - token_ids must be deduplicated
    /// - token_ids must not contain any IDs which are currently in
    ///   the claims queue for ADDR. This can be ensured by requiring
    ///   that claims are completed before the tokens may be restaked.
    pub fn create_nft_claims(
        &self,
        storage: &mut dyn Storage,
        addr: &Addr,
        token_ids: Vec<String>,
        release_at: Expiration,
    ) -> StdResult<()> {
        token_ids
            .into_iter()
            .map(|token_id| self.0.save(storage, (addr, &token_id), &release_at))
            .collect::<StdResult<Vec<_>>>()?;
        Ok(())
    }

    /// This iterates over all claims for the given IDs, removing them if they
    /// are all mature and erroring if any is not.
    pub fn claim_nfts(
        &self,
        storage: &mut dyn Storage,
        addr: &Addr,
        token_ids: &[String],
        block: &BlockInfo,
    ) -> Result<(), NftClaimError> {
        token_ids
            .iter()
            .map(|token_id| -> Result<(), NftClaimError> {
                match self.0.may_load(storage, (addr, token_id)) {
                    Ok(Some(expiration)) => {
                        // if claim is expired, continue
                        if expiration.is_expired(block) {
                            Ok(())
                        } else {
                            // if claim is not expired, error
                            Err(NftClaimError::NotReady {})
                        }
                    }
                    // if claim is not found, error
                    Ok(None) => Err(NftClaimError::NotFound {
                        token_id: token_id.clone(),
                    }),
                    Err(e) => Err(e.into()),
                }
            })
            .collect::<Result<Vec<_>, NftClaimError>>()?;

        // remove all now that we've confirmed they're mature
        token_ids
            .iter()
            .for_each(|token_id| self.0.remove(storage, (addr, token_id)));

        Ok(())
    }

    pub fn query_claims<Q: CustomQuery>(
        &self,
        deps: Deps<Q>,
        address: &Addr,
        start_after: Option<&String>,
        limit: Option<u32>,
    ) -> StdResult<NftClaimsResponse> {
        let limit = limit.map(|l| l as usize).unwrap_or(usize::MAX);
        let start = start_after.map(Bound::<&String>::exclusive);

        let nft_claims = self
            .0
            .prefix(address)
            .range(deps.storage, start, None, Order::Ascending)
            .take(limit)
            .map(|item| item.map(|(token_id, v)| NftClaim::new(token_id, v)))
            .collect::<StdResult<Vec<_>>>()?;

        Ok(NftClaimsResponse { nft_claims })
    }
}

#[cfg(test)]
mod test {
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env},
        Order,
    };

    use super::*;
    const TEST_BAYC_TOKEN_ID: &str = "BAYC";
    const TEST_CRYPTO_PUNKS_TOKEN_ID: &str = "CRYPTOPUNKS";
    const TEST_EXPIRATION: Expiration = Expiration::AtHeight(10);

    #[test]
    fn can_create_claim() {
        let claim = NftClaim::new(TEST_BAYC_TOKEN_ID.to_string(), TEST_EXPIRATION);
        assert_eq!(claim.token_id, TEST_BAYC_TOKEN_ID.to_string());
        assert_eq!(claim.release_at, TEST_EXPIRATION);
    }

    #[test]
    fn can_create_claims() {
        let deps = mock_dependencies();
        let claims = NftClaims::new("claims");
        // Assert that claims creates a map and there are no keys in the map.
        assert_eq!(
            claims
                .0
                .range_raw(&deps.storage, None, None, Order::Ascending)
                .collect::<StdResult<Vec<_>>>()
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn check_create_claim_updates_map() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_BAYC_TOKEN_ID.into()],
                TEST_EXPIRATION,
            )
            .unwrap();

        // Assert that claims creates a map and there is one claim for the address.
        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();
        assert_eq!(saved_claims.len(), 1);
        assert_eq!(saved_claims[0].0, TEST_BAYC_TOKEN_ID.to_string());
        assert_eq!(saved_claims[0].1, TEST_EXPIRATION);

        // Adding another claim to same address, make sure that both claims are saved.
        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.into()],
                TEST_EXPIRATION,
            )
            .unwrap();

        // Assert that both claims exist for the address.
        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();
        assert_eq!(saved_claims.len(), 2);
        assert_eq!(saved_claims[0].0, TEST_BAYC_TOKEN_ID.to_string());
        assert_eq!(saved_claims[0].1, TEST_EXPIRATION);
        assert_eq!(saved_claims[1].0, TEST_CRYPTO_PUNKS_TOKEN_ID.to_string());
        assert_eq!(saved_claims[1].1, TEST_EXPIRATION);

        // Adding another claim to different address, make sure that other address only has one claim.
        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr2"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()],
                TEST_EXPIRATION,
            )
            .unwrap();

        // Assert that both claims exist for the address.
        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();

        let saved_claims_addr2 = claims
            .0
            .prefix(&Addr::unchecked("addr2"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();
        assert_eq!(saved_claims.len(), 2);
        assert_eq!(saved_claims_addr2.len(), 1);
    }

    #[test]
    fn test_claim_tokens_with_no_claims() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        let env = mock_env();
        let error = claims
            .claim_nfts(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                &["404".to_string()],
                &env.block,
            )
            .unwrap_err();
        assert_eq!(
            error,
            NftClaimError::NotFound {
                token_id: "404".to_string()
            }
        );

        claims
            .claim_nfts(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                &[],
                &mock_env().block,
            )
            .unwrap();
        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range_raw(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();

        assert_eq!(saved_claims.len(), 0);
    }

    #[test]
    fn test_claim_tokens_with_no_released_claims() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()],
                Expiration::AtHeight(10),
            )
            .unwrap();

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_BAYC_TOKEN_ID.to_string()],
                Expiration::AtHeight(100),
            )
            .unwrap();

        let mut env = mock_env();
        env.block.height = 0;
        // the address has two claims however they are both not expired
        let error = claims
            .claim_nfts(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                &[
                    TEST_CRYPTO_PUNKS_TOKEN_ID.to_string(),
                    TEST_BAYC_TOKEN_ID.to_string(),
                ],
                &env.block,
            )
            .unwrap_err();
        assert_eq!(error, NftClaimError::NotReady {});

        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();

        assert_eq!(saved_claims.len(), 2);
        assert_eq!(saved_claims[0].0, TEST_BAYC_TOKEN_ID.to_string());
        assert_eq!(saved_claims[0].1, Expiration::AtHeight(100));
        assert_eq!(saved_claims[1].0, TEST_CRYPTO_PUNKS_TOKEN_ID.to_string());
        assert_eq!(saved_claims[1].1, Expiration::AtHeight(10));
    }

    #[test]
    fn test_claim_tokens_with_one_released_claim() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_BAYC_TOKEN_ID.to_string()],
                Expiration::AtHeight(10),
            )
            .unwrap();

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()],
                Expiration::AtHeight(100),
            )
            .unwrap();

        let mut env = mock_env();
        env.block.height = 20;
        // the address has two claims and the first one can be released
        claims
            .claim_nfts(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                &[TEST_BAYC_TOKEN_ID.to_string()],
                &env.block,
            )
            .unwrap();

        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();

        assert_eq!(saved_claims.len(), 1);
        assert_eq!(saved_claims[0].0, TEST_CRYPTO_PUNKS_TOKEN_ID.to_string());
        assert_eq!(saved_claims[0].1, Expiration::AtHeight(100));
    }

    #[test]
    fn test_claim_tokens_with_all_released_claims() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_BAYC_TOKEN_ID.to_string()],
                Expiration::AtHeight(10),
            )
            .unwrap();

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()],
                Expiration::AtHeight(100),
            )
            .unwrap();

        let mut env = mock_env();
        env.block.height = 1000;
        // the address has two claims and both can be released
        claims
            .claim_nfts(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                &[
                    TEST_BAYC_TOKEN_ID.to_string(),
                    TEST_CRYPTO_PUNKS_TOKEN_ID.to_string(),
                ],
                &env.block,
            )
            .unwrap();

        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<_>>>()
            .unwrap();

        assert_eq!(saved_claims.len(), 0);
    }

    #[test]
    fn test_query_claims_returns_correct_claims() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()],
                Expiration::AtHeight(10),
            )
            .unwrap();

        let queried_claims = claims
            .query_claims(deps.as_ref(), &Addr::unchecked("addr"), None, None)
            .unwrap();
        let saved_claims = claims
            .0
            .prefix(&Addr::unchecked("addr"))
            .range(deps.as_mut().storage, None, None, Order::Ascending)
            .map(|item| item.map(|(token_id, v)| NftClaim::new(token_id, v)))
            .collect::<StdResult<Vec<_>>>()
            .unwrap();

        assert_eq!(queried_claims.nft_claims, saved_claims);
    }

    #[test]
    fn test_query_claims_returns_correct_claims_paginated() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![
                    TEST_BAYC_TOKEN_ID.to_string(),
                    TEST_CRYPTO_PUNKS_TOKEN_ID.to_string(),
                ],
                Expiration::AtHeight(10),
            )
            .unwrap();

        let queried_claims = claims
            .query_claims(deps.as_ref(), &Addr::unchecked("addr"), None, None)
            .unwrap();
        assert_eq!(
            queried_claims.nft_claims,
            vec![
                NftClaim::new(TEST_BAYC_TOKEN_ID.to_string(), Expiration::AtHeight(10)),
                NftClaim::new(
                    TEST_CRYPTO_PUNKS_TOKEN_ID.to_string(),
                    Expiration::AtHeight(10)
                ),
            ]
        );

        let queried_claims = claims
            .query_claims(deps.as_ref(), &Addr::unchecked("addr"), None, Some(1))
            .unwrap();
        assert_eq!(
            queried_claims.nft_claims,
            vec![NftClaim::new(
                TEST_BAYC_TOKEN_ID.to_string(),
                Expiration::AtHeight(10)
            ),]
        );

        let queried_claims = claims
            .query_claims(
                deps.as_ref(),
                &Addr::unchecked("addr"),
                Some(&TEST_BAYC_TOKEN_ID.to_string()),
                None,
            )
            .unwrap();
        assert_eq!(
            queried_claims.nft_claims,
            vec![NftClaim::new(
                TEST_CRYPTO_PUNKS_TOKEN_ID.to_string(),
                Expiration::AtHeight(10)
            )]
        );

        let queried_claims = claims
            .query_claims(
                deps.as_ref(),
                &Addr::unchecked("addr"),
                Some(&TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()),
                None,
            )
            .unwrap();
        assert_eq!(queried_claims.nft_claims.len(), 0);
    }

    #[test]
    fn test_query_claims_returns_empty_for_non_existent_user() {
        let mut deps = mock_dependencies();
        let claims = NftClaims::new("claims");

        claims
            .create_nft_claims(
                deps.as_mut().storage,
                &Addr::unchecked("addr"),
                vec![TEST_CRYPTO_PUNKS_TOKEN_ID.to_string()],
                Expiration::AtHeight(10),
            )
            .unwrap();

        let queried_claims = claims
            .query_claims(deps.as_ref(), &Addr::unchecked("addr2"), None, None)
            .unwrap();

        assert_eq!(queried_claims.nft_claims.len(), 0);
    }
}
