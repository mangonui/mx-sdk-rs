#![allow(dead_code)]
#![allow(clippy::all)]

use multiversx_sc::proxy_imports::*;

pub struct RegistryLifecycleProxy;

impl<Env, From, To, Gas> TxProxyTrait<Env, From, To, Gas> for RegistryLifecycleProxy
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    type TxProxyMethods = RegistryLifecycleProxyMethods<Env, From, To, Gas>;

    fn proxy_methods(self, tx: Tx<Env, From, To, (), Gas, (), ()>) -> Self::TxProxyMethods {
        RegistryLifecycleProxyMethods { wrapped_tx: tx }
    }
}

pub struct RegistryLifecycleProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    wrapped_tx: Tx<Env, From, To, (), Gas, (), ()>,
}

#[rustfmt::skip]
impl<Env, From, To, Gas> RegistryLifecycleProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    pub fn retire_issuance_lot(
        self,
        lot_id: ManagedBuffer<Env::Api>,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("retireIssuanceLot")
            .argument(&lot_id)
            .original_result()
    }

    pub fn reverse_issuance_lot(
        self,
        lot_id: ManagedBuffer<Env::Api>,
        reversed_amount_scaled: BigUint<Env::Api>,
        replacement_lot_id: ManagedBuffer<Env::Api>,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("reverseIssuanceLot")
            .argument(&lot_id)
            .argument(&reversed_amount_scaled)
            .argument(&replacement_lot_id)
            .original_result()
    }
}
