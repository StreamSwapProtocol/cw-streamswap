use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    Addr, Decimal, Decimal256, Fraction, StdResult, Storage, Timestamp, Uint128, Uint256,
};
use cw_storage_plus::Map;

use crate::state::{Position, Status, Stream, StreamId, POSITIONS, STREAMS};

#[cw_serde]
pub struct StreamV0_2_0 {
    /// Name of the stream.
    pub name: String,
    /// Destination for the earned token_in.
    pub treasury: Addr,
    /// URL for more information about the stream.
    pub url: Option<String>,
    /// Proportional distribution variable to calculate the distribution of in token_out to buyers.
    pub dist_index: Decimal256,
    /// last updated time of stream.
    pub last_updated: Timestamp,
    /// denom of the `token_out`.
    pub out_denom: String,
    /// total number of `token_out` to be sold during the continuous stream.
    pub out_supply: Uint128,
    /// total number of remaining out tokens at the time of update.
    pub out_remaining: Uint128,
    /// denom of the `token_in`.
    pub in_denom: String,
    /// total number of `token_in` on the buy side at latest state.
    pub in_supply: Uint128,
    /// total number of `token_in` spent at latest state.
    pub spent_in: Uint128,
    /// total number of shares minted.
    pub shares: Uint128,
    /// start time when the token emission starts. in nanos.
    pub start_time: Timestamp,
    /// end time when the token emission ends.
    pub end_time: Timestamp,
    /// price at when latest distribution is triggered.
    pub current_streamed_price: Decimal,
    /// Status of the stream. Can be `Waiting`, `Active`, `Finalized`, `Paused` or `Canceled` for kill switch.
    pub status: Status,
    /// Date when the stream was paused.
    pub pause_date: Option<Timestamp>,
    /// Stream creation fee denom. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_creation_denom: String,
    /// Stream creation fee amount. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_creation_fee: Uint128,
    /// Stream swap fee in percent. Saved under here to avoid any changes in config to efect existing streams.
    pub stream_exit_fee_percent: Decimal,
}

#[cw_serde]
pub struct PositionV0_2_0 {
    /// creator of the position.
    pub owner: Addr,
    /// current amount of tokens in buy pool
    pub in_balance: Uint128,
    pub shares: Uint128,
    // index is used to calculate the distribution a position has
    pub index: Decimal256,
    pub last_updated: Timestamp,
    // total amount of `token_out` purchased in tokens at latest calculation
    pub purchased: Uint128,
    // pending purchased accumulates purchases after decimal truncation
    pub pending_purchase: Decimal256,
    // total amount of `token_in` spent tokens at latest calculation
    pub spent: Uint128,
    // operator can update position
    pub operator: Option<Addr>,
}

pub const OLD_STREAMS: Map<StreamId, StreamV0_2_0> = Map::new("stream");
pub const OLD_POSITIONS: Map<(StreamId, &Addr), PositionV0_2_0> = Map::new("positions");

pub fn migrate_v0_2_1(storage: &mut dyn Storage) -> StdResult<()> {
    // Migrate the state from v0.2.0 to v0.2.1
    let old_streams = OLD_STREAMS
        .range(storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    // migrate streams
    for (id, stream) in old_streams {
        let new_stream = Stream {
            name: stream.name,
            treasury: stream.treasury,
            url: stream.url,
            dist_index: stream.dist_index,
            last_updated: stream.last_updated,
            out_denom: stream.out_denom,
            out_supply: Uint256::from_uint128(stream.out_supply),
            out_remaining: Uint256::from_uint128(stream.out_remaining),
            in_denom: stream.in_denom,
            in_supply: Uint256::from_uint128(stream.in_supply),
            spent_in: Uint256::from_uint128(stream.spent_in),
            shares: Uint256::from_uint128(stream.shares),
            start_time: stream.start_time,
            end_time: stream.end_time,
            current_streamed_price: Decimal256::from_ratio(
                stream.current_streamed_price.numerator(),
                stream.current_streamed_price.denominator(),
            ),
            status: stream.status,
            pause_date: stream.pause_date,
            stream_creation_denom: stream.stream_creation_denom,
            stream_creation_fee: stream.stream_creation_fee,
            stream_exit_fee_percent: Decimal256::from_ratio(
                stream.stream_exit_fee_percent.numerator(),
                stream.stream_exit_fee_percent.denominator(),
            ),
        };
        STREAMS.save(storage, id, &new_stream)?;
    }

    // migrate positions
    let old_positions = OLD_POSITIONS
        .range(storage, None, None, cosmwasm_std::Order::Ascending)
        .collect::<StdResult<Vec<_>>>()?;

    for ((stream_id, owner), position) in old_positions {
        let new_position = Position {
            owner: position.owner,
            in_balance: Uint256::from_uint128(position.in_balance),
            shares: Uint256::from_uint128(position.shares),
            index: position.index,
            last_updated: position.last_updated,
            purchased: Uint256::from_uint128(position.purchased),
            pending_purchase: position.pending_purchase,
            spent: Uint256::from_uint128(position.spent),
            operator: position.operator,
        };
        POSITIONS.save(storage, (stream_id, &owner), &new_position)?;
    }

    Ok(())
}

#[cfg(test)]
mod test_migrate {
    use crate::migrate_v0_2_1::{
        migrate_v0_2_1, PositionV0_2_0, StreamV0_2_0, OLD_POSITIONS, OLD_STREAMS,
    };
    use crate::state::{Status, POSITIONS, STREAMS};
    use cosmwasm_std::{Addr, Decimal, Decimal256, StdResult, Timestamp, Uint128};

    #[test]
    fn test_migrate_v0_2_1() {
        use cosmwasm_std::testing::mock_dependencies;
        use std::str::FromStr;

        // Create a mock DepsMut instance
        let mut deps = mock_dependencies();

        // Set up the old streams and positions
        let old_streams = vec![
            (
                1u64,
                StreamV0_2_0 {
                    name: "Stream 1".to_string(),
                    treasury: Addr::unchecked("treasury1"),
                    url: Some("https://example.com/stream1".to_string()),
                    dist_index: Decimal256::from_str("0.0000000000088125").unwrap(),
                    last_updated: Timestamp::from_nanos(1721759663200729304),
                    out_denom: "token_out".to_string(),
                    out_supply: Uint128::new(79000000000),
                    out_remaining: Uint128::new(77236714240),
                    in_denom: "token_in".to_string(),
                    in_supply: Uint128::new(195633986821792674629),
                    spent_in: Uint128::new(4466013178207325371),
                    shares: Uint128::new(200100238743948046562),
                    start_time: Timestamp::from_nanos(1721759440000000000),
                    end_time: Timestamp::from_nanos(1721769440000000000),
                    current_streamed_price: Decimal::from_str("2544566918.67091152427620297")
                        .unwrap(),
                    status: Status::Active,
                    pause_date: None,
                    stream_creation_denom: "stream_creation_denom".to_string(),
                    stream_creation_fee: Uint128::new(10000000),
                    stream_exit_fee_percent: Decimal::from_str("0.1").unwrap(),
                },
            ),
            // Add more old streams if needed
        ];

        let old_positions = vec![
            (
                (1u64, Addr::unchecked("owner1")),
                PositionV0_2_0 {
                    owner: Addr::unchecked("owner1"),
                    in_balance: Uint128::new(100000000),
                    shares: Uint128::new(100000000),
                    index: Decimal256::from_ratio(1u128, 2u128),
                    last_updated: Timestamp::from_nanos(1234567890),
                    purchased: Uint128::new(50000000),
                    pending_purchase: Decimal256::from_ratio(1u128, 2u128),
                    spent: Uint128::new(50000000),
                    operator: None,
                },
            ),
            // Add more old positions if needed
        ];

        // Save the old streams and positions to storage
        for (id, stream) in old_streams.clone() {
            OLD_STREAMS
                .save(deps.as_mut().storage, id, &stream)
                .unwrap();
        }

        for ((stream_id, owner), position) in old_positions.clone() {
            OLD_POSITIONS
                .save(deps.as_mut().storage, (stream_id, &owner), &position)
                .unwrap();
        }

        // Call the migrate_v0_2_1 function
        migrate_v0_2_1(&mut deps.storage).unwrap();

        // Assert that the old streams and positions have been migrated to the new format
        let new_streams: StdResult<Vec<_>> = STREAMS
            .range(
                deps.as_ref().storage,
                None,
                None,
                cosmwasm_std::Order::Ascending,
            )
            .collect();
        assert_eq!(new_streams.unwrap().len(), old_streams.len());

        let new_positions: StdResult<Vec<_>> = POSITIONS
            .range(
                deps.as_ref().storage,
                None,
                None,
                cosmwasm_std::Order::Ascending,
            )
            .collect();
        assert_eq!(new_positions.unwrap().len(), old_positions.len());
    }
}
