use many_error::ManyError;
use many_identity::Address;
use many_modules::data::{
    self, DataGetInfoArgs, DataGetInfoReturns, DataInfoArgs, DataInfoReturns, DataQueryArgs,
    DataQueryReturns,
};

pub struct DataModuleImpl;

impl data::DataModuleBackend for DataModuleImpl {
    fn info(&self, _sender: &Address, _args: DataInfoArgs) -> Result<DataInfoReturns, ManyError> {
        Ok(DataInfoReturns { indices: vec![] })
    }

    fn get_info(
        &self,
        _sender: &Address,
        _args: DataGetInfoArgs,
    ) -> Result<DataGetInfoReturns, ManyError> {
        Ok(DataGetInfoReturns::new())
    }

    fn query(
        &self,
        _sender: &Address,
        _args: DataQueryArgs,
    ) -> Result<DataQueryReturns, ManyError> {
        Ok(DataQueryReturns::new())
    }
}
