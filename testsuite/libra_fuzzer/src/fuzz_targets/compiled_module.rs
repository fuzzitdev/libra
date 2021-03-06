// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::FuzzTargetImpl;
use proptest::prelude::*;
use proptest_helpers::ValueGenerator;
use vm::file_format::{CompiledModule, CompiledModuleMut};

#[derive(Clone, Debug, Default)]
pub struct CompiledModuleTarget;

impl FuzzTargetImpl for CompiledModuleTarget {
    fn name(&self) -> &'static str {
        module_name!()
    }

    fn description(&self) -> &'static str {
        "VM CompiledModule (custom deserializer)"
    }

    fn generate(&self, gen: &mut ValueGenerator) -> Vec<u8> {
        let value = gen.generate(any_with::<CompiledModuleMut>(16));
        let mut out = vec![];
        value
            .serialize(&mut out)
            .expect("serialization should work");
        out
    }

    fn fuzz(&self, data: &[u8]) {
        // Errors are OK -- the fuzzer cares about panics and OOMs. Note that
        // `CompiledModule::deserialize` also runs the bounds checker, which is desirable here.
        let _ = CompiledModule::deserialize(data);
    }
}
