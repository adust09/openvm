use std::{cmp::max, iter::once, marker::PhantomData};

use ndarray::s;
use openvm_circuit_primitives::{
    bitwise_op_lookup::BitwiseOperationLookupBus,
    encoder::Encoder,
    utils::{not, select},
    SubAir,
};
use openvm_sha2_air::{
    compose, Sha2BlockHasherSubairConfig, Sha2BlockHasherSubAir, Sha2DigestColsRef, Sha2RoundColsRef,
};
use openvm_stark_backend::{
    interaction::{BusIndex, InteractionBuilder, PermutationCheckBus},
    p3_air::{Air, AirBuilder, BaseAir},
    p3_field::{Field, FieldAlgebra},
    p3_matrix::Matrix,
    rap::{BaseAirWithPublicValues, PartitionedBaseAir},
};

use crate::{
    MessageType, Sha2BlockHasherVmDigestColsRef, Sha2BlockHasherVmRoundColsRef, INNER_OFFSET,
};

pub struct Sha2BlockHasherVmAir<C: Sha2BlockHasherSubairConfig> {
    pub inner: Sha2BlockHasherSubAir<C>,
    pub sha2_bus: PermutationCheckBus,
}

impl<C: Sha2BlockHasherSubairConfig> Sha2BlockHasherVmAir<C> {
    pub fn new(
        bitwise_lookup_bus: BitwiseOperationLookupBus,
        inner_bus_idx: BusIndex,
        shared_bus_idx: BusIndex,
    ) -> Self {
        Self {
            inner: Sha2BlockHasherSubAir::new(bitwise_lookup_bus, inner_bus_idx),
            sha2_bus: PermutationCheckBus::new(shared_bus_idx),
        }
    }
}

impl<F: Field, C: Sha2BlockHasherSubairConfig> BaseAirWithPublicValues<F> for Sha2BlockHasherVmAir<C> {}
impl<F: Field, C: Sha2BlockHasherSubairConfig> PartitionedBaseAir<F> for Sha2BlockHasherVmAir<C> {}
impl<F: Field, C: Sha2BlockHasherSubairConfig> BaseAir<F> for Sha2BlockHasherVmAir<C> {
    fn width(&self) -> usize {
        C::WIDTH
    }
}

impl<AB: InteractionBuilder, C: Sha2BlockHasherSubairConfig> Air<AB> for Sha2BlockHasherVmAir<C> {
    fn eval(&self, builder: &mut AB) {
        self.inner.eval(builder, INNER_OFFSET);
        self.eval_interactions(builder);
        self.eval_request_id(builder);
    }
}

impl<C: Sha2BlockHasherSubairConfig> Sha2BlockHasherVmAir<C> {
    fn eval_interactions<AB: InteractionBuilder>(&self, builder: &mut AB) {
        let main = builder.main();
        let local_slice = main.row_slice(0);
        let next_slice = main.row_slice(1);

        let local =
            Sha2BlockHasherVmDigestColsRef::<AB::Var>::from::<C>(&local_slice[..C::DIGEST_WIDTH]);

        // Receive (STATE, request_id, prev_state_as_u16s, new_state) on the sha2 bus
        self.sha2_bus.receive(
            builder,
            [
                AB::Expr::from_canonical_u8(MessageType::State as u8),
                (*local.request_id).into(),
            ]
            .into_iter()
            .chain(local.inner.prev_hash.flatten().map(|x| (*x).into()))
            .chain(local.inner.final_hash.flatten().map(|x| (*x).into())),
            *local.inner.flags.is_digest_row,
        );

        let local =
            Sha2BlockHasherVmRoundColsRef::<AB::Var>::from::<C>(&local_slice[..C::ROUND_WIDTH]);
        let next =
            Sha2BlockHasherVmRoundColsRef::<AB::Var>::from::<C>(&next_slice[..C::ROUND_WIDTH]);

        let is_local_first_row = self
            .inner
            .row_idx_encoder
            .contains_flag::<AB>(&local.inner.flags.row_idx.to_slice().unwrap(), &[0]);

        // Copied from old Sha256VmChip:
        // https://github.com/openvm-org/openvm/blob/c2e376e6059c8bbf206736cf01d04cda43dfc42d/extensions/sha256/circuit/src/sha256_chip/air.rs#L310C1-L318C1
        let get_ith_byte = |i: usize| {
            let word_idx = i / C::ROUNDS_PER_ROW;
            let word: Vec<AB::Var> = local
                .inner
                .message_schedule
                .w
                .row(word_idx)
                .into_iter()
                .map(|x| *x)
                .collect::<Vec<_>>();
            // Need to reverse the byte order to match the endianness of the memory
            let byte_idx = 4 - i % 4 - 1;
            compose::<AB::Expr>(&word[byte_idx * 8..(byte_idx + 1) * 8], 1)
        };

        let row_0_message_bits = local
            .inner
            .message_schedule
            .w
            .iter()
            .map(|x| (*x).into())
            .collect::<Vec<_>>();
        let row_1_message_bits = next
            .inner
            .message_schedule
            .w
            .iter()
            .map(|x| (*x).into())
            .collect::<Vec<_>>();

        let row_0_message_bytes = (0..row_0_message_bits.len() / 8)
            .map(|i| get_ith_byte(i))
            .collect::<Vec<_>>();
        let row_1_message_bytes = (0..row_1_message_bits.len() / 8)
            .map(|i| get_ith_byte(i))
            .collect::<Vec<_>>();

        // Receive (MESSAGE_1, request_id, first_half_of_message) on the sha2 bus
        self.sha2_bus.send(
            builder,
            [
                AB::Expr::from_canonical_u8(MessageType::Message1 as u8),
                (*local.request_id).into(),
            ]
            .into_iter()
            .chain(row_0_message_bytes)
            .chain(row_1_message_bytes),
            is_local_first_row * *local.inner.flags.is_first_4_rows, /* is_first_4_rows checks
                                                                      * if
                                                                      * the row is enabled */
        );

        let is_local_third_row = self
            .inner
            .row_idx_encoder
            .contains_flag::<AB>(&local.inner.flags.row_idx.to_slice().unwrap(), &[2]);

        let row_2_message_bits = local
            .inner
            .message_schedule
            .w
            .iter()
            .map(|x| (*x).into())
            .collect::<Vec<_>>();
        let row_3_message_bits = next
            .inner
            .message_schedule
            .w
            .iter()
            .map(|x| (*x).into())
            .collect::<Vec<_>>();

        let row_2_message_bytes = (0..row_2_message_bits.len() / 8)
            .map(|i| get_ith_byte(i))
            .collect::<Vec<_>>();
        let row_3_message_bytes = (0..row_3_message_bits.len() / 8)
            .map(|i| get_ith_byte(i))
            .collect::<Vec<_>>();

        // Send (MESSAGE_2, request_id, second_half_of_message) to the sha2 bus
        self.sha2_bus.send(
            builder,
            [
                AB::Expr::from_canonical_u8(MessageType::Message2 as u8),
                (*local.request_id).into(),
            ]
            .into_iter()
            .chain(row_2_message_bytes)
            .chain(row_3_message_bytes),
            is_local_third_row * *local.inner.flags.is_first_4_rows, /* is_first_4_rows checks
                                                                      * if
                                                                      * the row is enabled */
        );
    }

    fn eval_request_id<AB: InteractionBuilder>(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.row_slice(0);
        let next = main.row_slice(1);

        // doesn't matter if we use round or digest cols here, since we only access
        // request_id and inner.flags.is_last block, which are common to both
        // field
        let local = Sha2BlockHasherVmRoundColsRef::<AB::Var>::from::<C>(&local[..C::WIDTH]);
        let next = Sha2BlockHasherVmRoundColsRef::<AB::Var>::from::<C>(&next[..C::WIDTH]);

        builder.when_transition().assert_eq(
            *next.request_id,
            *local.request_id * (AB::Expr::ONE - *local.inner.flags.is_last_block),
        );
    }
}
