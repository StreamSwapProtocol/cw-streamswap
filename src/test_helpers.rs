use crate::contract::{execute, instantiate, query};
use cosmwasm_std::{to_json_binary, Addr, Api, Binary, BlockInfo, Empty, Querier, Storage};
use cw_multi_test::{
    error::AnyResult, AppResponse, Contract, ContractWrapper, CosmosRouter, Stargate,
};
use osmosis_std::types::cosmos::base::v1beta1::Coin;
use osmosis_std::types::osmosis::poolmanager::v1beta1::{NumPoolsResponse, Params, ParamsResponse};

pub fn contract_streamswap() -> Box<dyn Contract<Empty>> {
    Box::new(ContractWrapper::new(execute, instantiate, query))
}
pub struct MyStargateKeeper {}

impl Stargate for MyStargateKeeper {
    fn execute<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _sender: Addr,
        _type_url: String,
        _value: Binary,
    ) -> AnyResult<AppResponse> {
        /*
        if type_url == *"/osmosis.concentratedliquidity.poolmodel.concentrated.v1beta1.MsgCreateConcentratedPool" {
            let parsed_msg: Result<MsgCreateConcentratedPool, DecodeError> = Message::decode(value.as_slice());
            if let Ok(msg) = parsed_msg {
                let collection = Collection {
                    denom: Some(Denom {
                        creator: sender.to_string(),
                        data: msg.data,
                        name: msg.name,
                        id: msg.id,
                        preview_uri: msg.preview_uri,
                        description: msg.description,
                        schema: msg.schema,
                        symbol: msg.symbol,
                        uri: msg.uri,
                        uri_hash: msg.uri_hash,
                        royalty_receivers: msg.royalty_receivers,
                    }),
                    onfts: vec![],
                };
                let key = format!("collections:{}:{}", COLLECTION_PREFIX, sender);
                let serialized_collection =
                    to_json_binary(&collection).expect("Failed to serialize Collection");
                storage.set(key.as_bytes(), &serialized_collection);
            }
        }

         */
        Ok(AppResponse::default())
    }

    fn query(
        &self,
        _api: &dyn Api,
        _storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        path: String,
        data: Binary,
    ) -> AnyResult<Binary> {
        match path.as_str() {
            "/osmosis.poolmanager.v1beta1.Query/Params" => {
                let params = ParamsResponse {
                    params: Some(Params {
                        pool_creation_fee: vec![Coin {
                            denom: "uosmo".to_string(),
                            amount: "1000000".to_string(),
                        }],
                        taker_fee_params: None,
                        authorized_quote_denoms: vec![],
                    }),
                };
                return Ok(to_json_binary(&params)?);
            }
            "/osmosis.poolmanager.v1beta1.Query/NumPools" => {
                let res = NumPoolsResponse { num_pools: 1 };
                return Ok(to_json_binary(&res)?);
            }
            _ => return Ok(data),
        }
    }
}
