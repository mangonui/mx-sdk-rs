#![allow(dead_code)]
#![allow(clippy::all)]

use multiversx_sc::proxy_imports::*;

pub struct BufferPoolProxy;

impl<Env, From, To, Gas> TxProxyTrait<Env, From, To, Gas> for BufferPoolProxy
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    type TxProxyMethods = BufferPoolProxyMethods<Env, From, To, Gas>;

    fn proxy_methods(self, tx: Tx<Env, From, To, (), Gas, (), ()>) -> Self::TxProxyMethods {
        BufferPoolProxyMethods { wrapped_tx: tx }
    }
}

pub struct BufferPoolProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    wrapped_tx: Tx<Env, From, To, (), Gas, (), ()>,
}

#[rustfmt::skip]
impl<Env, From, To, Gas> BufferPoolProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    pub fn get_buffer_record(
        self,
        project_id: ManagedBuffer<Env::Api>,
    ) -> TxTypedCall<
        Env,
        From,
        To,
        NotPayable,
        Gas,
        OptionalValue<mrv_buffer_pool::BufferRecord<Env::Api>>,
    > {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getBufferRecord")
            .argument(&project_id)
            .original_result()
    }

    pub fn get_total_pool_balance(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalPoolBalance")
            .original_result()
    }

    pub fn deposit_buffer_credits(
        self,
        project_id: ManagedBuffer<Env::Api>,
        amount_scaled: BigUint<Env::Api>,
        monitoring_period_n: u64,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("depositBufferCredits")
            .argument(&project_id)
            .argument(&amount_scaled)
            .argument(&monitoring_period_n)
            .original_result()
    }
}
