use std::marker::PhantomData;

use halo2_proofs::{
    arithmetic::FieldExt,
    circuit::{AssignedCell, Layouter, SimpleFloorPlanner, Value},
    dev::MockProver,
    pasta::Fp,
    plonk::{Advice, Circuit, Column, ConstraintSystem, Error, Instance, Selector},
    poly::Rotation,
};

#[derive(Debug, Clone)]
struct FiboConfig {
    advice: Column<Advice>,
    selector: Selector,
    instance: Column<Instance>,
}

#[derive(Debug)]
struct FiboChip<F: FieldExt> {
    config: FiboConfig,
    marker: PhantomData<F>,
}

impl<F: FieldExt> FiboChip<F> {
    pub fn construct(config: FiboConfig) -> Self {
        Self {
            config,
            marker: PhantomData::default(),
        }
    }

    pub fn configure(
        meta: &mut ConstraintSystem<F>,
        advice: Column<Advice>,
        instance: Column<Instance>,
    ) -> FiboConfig {
        meta.enable_equality(advice);
        meta.enable_equality(instance);

        let selector = meta.selector();

        meta.create_gate("fibonacci", |meta| {
            //
            // advice | selector
            //   a    |    s
            //   b    |
            //   c    |
            //

            let a = meta.query_advice(advice, Rotation(0));
            let b = meta.query_advice(advice, Rotation(1));
            let c = meta.query_advice(advice, Rotation(2));

            let s = meta.query_selector(selector);

            vec![s * (a + b - c)]
        });

        FiboConfig {
            advice,
            selector,
            instance,
        }
    }

    pub fn assign_first_row(
        &self,
        mut layouter: impl Layouter<F>,
        a: Value<F>,
        b: Value<F>,
    ) -> Result<(AssignedCell<F, F>, AssignedCell<F, F>, AssignedCell<F, F>), Error> {
        layouter.assign_region(
            || "first row",
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                let a_cell = region.assign_advice(|| "a", self.config.advice, 0, || a)?;
                let b_cell = region.assign_advice(|| "b", self.config.advice, 1, || b)?;
                let c_cell = region.assign_advice(|| "c", self.config.advice, 2, || a + b)?;

                Ok((a_cell, b_cell, c_cell))
            },
        )
    }

    pub fn assign_row(
        &self,
        mut layouter: impl Layouter<F>,
        prev_b: AssignedCell<F, F>,
        prev_c: AssignedCell<F, F>,
    ) -> Result<(AssignedCell<F, F>, AssignedCell<F, F>), Error> {
        layouter.assign_region(
            || "next row",
            |mut region| {
                self.config.selector.enable(&mut region, 0)?;

                let _a_cell = prev_b.copy_advice(|| "a", &mut region, self.config.advice, 0)?;
                let b_cell = prev_c.copy_advice(|| "b", &mut region, self.config.advice, 1)?;

                let c_cell = region.assign_advice(
                    || "c",
                    self.config.advice,
                    2,
                    || prev_b.value().copied() + prev_c.value().copied(),
                )?;

                Ok((b_cell, c_cell))
            },
        )
    }

    pub fn expose_public(
        &self,
        mut layouter: impl Layouter<F>,
        cell: AssignedCell<F, F>,
        row: usize,
    ) -> Result<(), Error> {
        layouter.constrain_instance(cell.cell(), self.config.instance, row)
    }
}

#[derive(Debug, Default)]
struct FiboCircuit<F: FieldExt> {
    pub a: Value<F>,
    pub b: Value<F>,
}

impl<F: FieldExt> Circuit<F> for FiboCircuit<F> {
    type Config = FiboConfig;

    type FloorPlanner = SimpleFloorPlanner;

    fn without_witnesses(&self) -> Self {
        Self::default()
    }

    fn configure(meta: &mut ConstraintSystem<F>) -> Self::Config {
        let advice = meta.advice_column();
        let instance = meta.instance_column();

        FiboChip::configure(meta, advice, instance)
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<F>,
    ) -> Result<(), Error> {
        let cs = FiboChip::construct(config);

        let (_, mut prev_b, mut prev_c) =
            cs.assign_first_row(layouter.namespace(|| "first row"), self.a, self.b)?;

        for _ in 3..10 {
            let (b, c) = cs.assign_row(
                layouter.namespace(|| "next row"),
                prev_b.clone(),
                prev_c.clone(),
            )?;
            prev_b = b;
            prev_c = c;
        }

        cs.expose_public(layouter.namespace(|| "expose public"), prev_c, 0)?;

        Ok(())
    }
}

fn main() {
    let k = 5;

    let fibo_circuit = FiboCircuit {
        a: Value::known(Fp::from(1)),
        b: Value::known(Fp::from(1)),
    };
    let public_input = vec![Fp::from(55)];

    let prover = MockProver::run(k, &fibo_circuit, vec![public_input]).unwrap();
    prover.assert_satisfied();

    // Plot the circuit
    use plotters::prelude::*;
    let root = BitMapBackend::new("fib-2-layout.png", (1024, 768)).into_drawing_area();
    root.fill(&WHITE).unwrap();
    let root = root.titled("Fib 2 Layout", ("sans-serif", 60)).unwrap();

    let circuit = FiboCircuit {
        a: Value::known(Fp::from(1)),
        b: Value::known(Fp::from(1)),
    };
    halo2_proofs::dev::CircuitLayout::default()
        .render(13, &circuit, &root)
        .unwrap();
}
