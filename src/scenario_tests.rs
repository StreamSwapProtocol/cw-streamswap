#[cfg(test)]
mod tests {

    /*
    initial out supply: 50_000_000
    stream duration 1_000_000

    subscribe 1

    in: 25_000
    time: 1000 unix
    stage: 0,001
    new_dist: 50000
    dist_index: 50000/50000000 0,001

    subscribe 2

    in: 50_000
    time: 250_000
    stage: 0.25
    spent in:
    dist_index:

    withdraw 1

    withdraw 2

    subscribe 1 - load

    finalize

    exit 1
    exit 2
     */
    /*
    #[test]
    fn scenario_one() {
        // initial sell supply: 1_000_000
        let mut deps = mock_dependencies();

        // instantiate contract
        let msg = InstantiateMsg {
            min_stream_duration: Uint64::new(1000),
            min_duration_until_start_time: Uint64::new(1000),
            stream_creation_denom: "fee".to_string(),
            stream_creation_fee: Uint128::new(100),
            fee_collector: "collector".to_string(),
        };
        let info = mock_info("creator", &[]);
        let mut env = mock_env();
        env.block.time = Timestamp::from_nanos(0);
        let _ = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg = ExecuteMsg::CreateStream {
            treasury: "treasury".to_string(),
            name: "name".to_string(),
            url: "url".to_string(),
            in_denom: "in".to_string(),
            out_denom: "out".to_string(),
            out_supply: Uint128::new(1_000_000),
            start_time: Timestamp::from_nanos(1000),
            end_time: Timestamp::from_nanos(2000),
        };
        let funds = vec![coin(100, "fee"), coin(1_000_000, "out")];
        let info = mock_info("stream_creator", &funds);
        let _ = execute(deps.as_mut(), env, info, msg).unwrap();
    }

     */
}
