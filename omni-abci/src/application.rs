use omni::message::decode_request_from_cose_sign1;
use omni::server::RequestHandler;
use tendermint_abci::Application;
use tendermint_proto::abci::{
    RequestApplySnapshotChunk, RequestBeginBlock, RequestCheckTx, RequestDeliverTx, RequestEcho,
    RequestEndBlock, RequestInfo, RequestInitChain, RequestLoadSnapshotChunk, RequestOfferSnapshot,
    RequestQuery, RequestSetOption, ResponseApplySnapshotChunk, ResponseBeginBlock,
    ResponseCheckTx, ResponseCommit, ResponseDeliverTx, ResponseEcho, ResponseEndBlock,
    ResponseFlush, ResponseInfo, ResponseInitChain, ResponseListSnapshots,
    ResponseLoadSnapshotChunk, ResponseOfferSnapshot, ResponseQuery, ResponseSetOption,
};

#[derive(Clone, Debug)]
struct OmniAbciApplication<H: RequestHandler + Clone + Send + Sync> {
    handler: H,
}

impl Application for OmniAbciApplication {
    fn check_tx(&self, request: RequestCheckTx) -> ResponseCheckTx {
        let tx = request.tx;
        let cose =
        decode_request_from_cose_sign1()
    }
}
