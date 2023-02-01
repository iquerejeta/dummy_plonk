// We begin implementing the plonk circuit. For sake of simplicity
// we only create two gadgets. ADD and MULT. Maybe in the future
// we will experiment with custom gates.
//
// Recall that plonk gates are defined by:
//
// q_L * a + q_R * b + q_O * c + q_M * ab + q_C = 0
//
// where a, b, c are the left, right and output wires of the gate.
//
// So, if we want to add as a constraint 5 * 3 = 15, then we would set
//
// q_L = q_R = q_C = 0, q_O = -1, q_M = 1
//
// To view this clearly, we represent each plonk gate by a [Scalar; 5],
// meaning that a circuit will be created by Vec<[Scalar; 5]>, with its
// length being the number of constraints we include in the circuit, e.g. :
//
//           | q_L |    |  0,  1, ... ,  0|
//           | q_R |    |  0,  1, ... ,  0|
// Circuit = | q_O | =  | -1, -1, ... , -1|
//           | q_M |    |  1,  0, ... ,  1|
//           | q_C |    |  0,  0, ... ,  0|
//
// The trace is the left, right and output values of each constraint.
//
// We also need to declare the connections between the wires. For that, we require
// the circuit designer to specify which two wires are connected. For sake
// of simplicity, we are only exposing an addition and a multiplication gate. Each
// one of these gates increases the total number of wires by 3, so it should be easy
// to keep the count in our examples.
#![allow(non_snake_case)]
use crate::polynomial::{Polynomial, PolynomialEvaluationPoints};
use bls12_381::Scalar;
use ff::PrimeField;
use std::collections::HashMap;
use crate::kzg10::Kzg10;

pub(crate) fn K1() -> Scalar {
    Scalar::from(7_u64)
}
pub(crate) fn K2() -> Scalar {
    Scalar::from(13_u64)
}

pub struct ComputationTrace {
    pub(crate) a: Vec<Scalar>,
    pub(crate) b: Vec<Scalar>,
    pub(crate) c: Vec<Scalar>
}

#[derive(Clone, Default)]
pub struct Constraints {
    qm: Vec<Scalar>,
    ql: Vec<Scalar>,
    qr: Vec<Scalar>,
    qo: Vec<Scalar>,
    qc: Vec<Scalar>,
}

#[derive(Clone)]
pub struct PlonkCircuit {
    pub extended_h_subgroup: Vec<Scalar>,
    pub constraints: Constraints,
    pub permutations: HashMap<usize, usize>,
    pub nr_wires: usize,
    pub nr_constraints: usize,
    pub powers_omega: Vec<Scalar>,
}

pub struct PlonkConstraintSystem(ComputationTrace, PlonkCircuit);

pub struct PreprocessedInput {
    pub kzg_set: Kzg10<128>, // We could make this generic, but it's only going to complicate the code.
    pub blinder_polynomial: Polynomial,
    pub constraints: PlonkCircuit,
    pub sigma_star: HashMap<usize, Scalar>,
    pub qm_x: Polynomial,
    pub ql_x: Polynomial,
    pub qr_x: Polynomial,
    pub qo_x: Polynomial,
    pub qc_x: Polynomial,
    pub qs1_x: Polynomial,
    pub qs2_x: Polynomial,
    pub qs3_x: Polynomial,
}

impl PlonkCircuit {
    pub fn init() -> Self {
        Self {
            constraints: Default::default(),
            permutations: Default::default(),
            nr_wires: 0,
            nr_constraints: 0,
            extended_h_subgroup: Default::default(),
            powers_omega: Vec::new(),
        }
    }
    pub fn add_gate(&mut self) {
        self.constraints.ql.push(Scalar::one());
        self.constraints.qr.push(Scalar::one());
        self.constraints.qo.push(Scalar::one().neg());
        self.constraints.qm.push(Scalar::zero());
        self.constraints.qc.push(Scalar::zero());

        // we extend the permutation with the identity permutation
        self.permutations.insert(self.nr_wires, self.nr_wires);
        self.permutations.insert(self.nr_wires + 1, self.nr_wires + 1);
        self.permutations.insert(self.nr_wires + 2, self.nr_wires + 2);

        self.nr_wires += 3;
        self.nr_constraints += 1;
    }

    pub fn mult_gate(&mut self) {
        self.constraints.qm.push(Scalar::one());
        self.constraints.qo.push(Scalar::one().neg());
        self.constraints.ql.push(Scalar::zero());
        self.constraints.qr.push(Scalar::zero());
        self.constraints.qc.push(Scalar::zero());

        // we extend the permutation with the identity permutation
        self.permutations.insert(self.nr_wires, self.nr_wires);
        self.permutations.insert(self.nr_wires + 1, self.nr_wires + 1);
        self.permutations.insert(self.nr_wires + 2, self.nr_wires + 2);

        self.nr_wires += 3;
        self.nr_constraints += 1;
    }

    // This should always be called after creating the gates.
    pub fn connect_wires(&mut self, in_wire: &usize, out_wire: &usize) {
        assert!(*in_wire < self.nr_wires && *out_wire < self.nr_wires, "The circuit does not have enough wires for these two. Max {0}, got {in_wire} and {out_wire}", self.nr_wires);
        let end = self.permutations.insert(*in_wire, *out_wire).unwrap(); // we know each key is populated
        self.permutations.insert(*out_wire, end);
    }

    pub fn lagrange_basis(&self, index: usize) -> Polynomial {
        let mut lb = Polynomial(vec![Scalar::from(1)]);
        for j in 0..self.extended_h_subgroup.len() {
            if index == j {
                continue;
            }
            lb *= &Polynomial(vec![self.extended_h_subgroup[j].neg(), Scalar::one()])
                * &(self.extended_h_subgroup[index] - self.extended_h_subgroup[j])
                    .invert()
                    .unwrap();
        }
        lb
    }

    pub fn compute_sigma_star(&self) -> HashMap<usize, Scalar> {
        self.permutations
            .iter()
            .map(|(index, value)| match index / self.nr_constraints {
                0 => {
                    return (
                        *index,
                        self.powers_omega[0]
                            .pow_vartime(&[(value % self.nr_constraints) as u64, 0, 0, 0]),
                    )
                }
                1 => {
                    return (
                        *index,
                        K1() * self.powers_omega[0].pow_vartime(&[
                            (value % self.nr_constraints) as u64,
                            0,
                            0,
                            0,
                        ]),
                    )
                }
                2 => {
                    return (
                        *index,
                        K2() * self.powers_omega[0].pow_vartime(&[
                            (value % self.nr_constraints) as u64,
                            0,
                            0,
                            0,
                        ]),
                    )
                }
                _ => {
                    panic!("well, this was unexpected")
                }
            })
            .collect::<HashMap<usize, Scalar>>()
    }

    pub fn setup(&mut self) -> PreprocessedInput {
        // For simplicity, we begin computing our extended subgroup H'. We need a nth root of unity with
        // n being the number of constraints. We compute this root of unity out of the 2^32nd
        // root of unity, g, which is provided as a constant in the underlying library. We do so
        // by calculating omega = g^{2^{32 - n}}.
        let omega =
            Scalar::root_of_unity().pow_vartime(&[1u64 << (32 - self.nr_constraints), 0, 0, 0]);

        self.powers_omega = vec![Scalar::one(); self.nr_constraints];
        self.powers_omega[0] = omega.clone();
        for i in 1..self.nr_constraints {
            self.powers_omega[i] = self.powers_omega[i-1] * omega;
        }

        assert_eq!(omega.pow_vartime(&[1u64 << self.nr_constraints as u64, 0, 0, 0]), Scalar::one());

        self.extended_h_subgroup = vec![Scalar::zero(); self.nr_constraints * 3];
        self.extended_h_subgroup[0] = self.powers_omega[0].clone();
        self.extended_h_subgroup[self.nr_constraints] = K1() * self.powers_omega[0];
        self.extended_h_subgroup[self.nr_constraints * 2] = K2() * self.powers_omega[0];

        for index in 1..self.nr_constraints {
            self.extended_h_subgroup[index] = self.extended_h_subgroup[index - 1] * self.powers_omega[0];
            self.extended_h_subgroup[index + self.nr_constraints] =
                self.extended_h_subgroup[index] * K1();
            self.extended_h_subgroup[index + self.nr_constraints * 2] =
                self.extended_h_subgroup[index] * K2();
        }

        // Next, we define the \sigma*
        let sigma_star = self.compute_sigma_star();

        // Now we create the permutation polynomials qs1, qs2 and qs3.
        let mut qs1_x = Polynomial::zero(self.nr_constraints);
        let mut qs2_x = Polynomial::zero(self.nr_constraints);
        let mut qs3_x = Polynomial::zero(self.nr_constraints);

        for i in 0..self.nr_constraints {
            let lp = self.lagrange_basis(i);
            qs1_x += &lp * sigma_star.get(&i).unwrap();
            qs2_x += &lp * sigma_star.get(&(self.nr_constraints + i)).unwrap();
            qs3_x += &lp * sigma_star.get(&(self.nr_constraints * 2 + i)).unwrap();
        }

        // Next we compute the selector polynomials. This is performed by interpolating
        // the pairs (g^i, q_i), for q_i being elements of the vectors, ql, qr, ..., qc.
        let ql_x = PolynomialEvaluationPoints(
            self.constraints.ql.iter().zip(self.powers_omega.iter()).map(|(element, power_w)| {
                (power_w.clone(), element.clone())
            }).collect()
        ).interpolate();

        let qr_x = PolynomialEvaluationPoints(
            self.constraints
                .qr
                .iter().zip(self.powers_omega.iter())
                .map(|(element, power_w)| {
                    (power_w.clone(), element.clone())
                })
                .collect(),
        )
        .interpolate();

        let qc_x = PolynomialEvaluationPoints(
            self.constraints
                .qc
                .iter().zip(self.powers_omega.iter())
                .map(|(element, power_w)| (power_w.clone(), element.clone()))
                .collect(),
        )
        .interpolate();

        let qm_x = PolynomialEvaluationPoints(
            self.constraints
                .qm
                .iter().zip(self.powers_omega.iter())
                .map(|(element, power_w)| (power_w.clone(), element.clone()))
                .collect(),
        )
        .interpolate();

        let qo_x = PolynomialEvaluationPoints(
            self.constraints
                .qo
                .iter().zip(self.powers_omega.iter())
                .map(|(element, power_w)| (power_w.clone(), element.clone()))
                .collect(),
        )
        .interpolate();

        let mut blinder_vec = vec![Scalar::zero(); (1 << self.nr_constraints) + 1];
        blinder_vec[0] = Scalar::one().neg();
        blinder_vec[1 << self.nr_constraints] = Scalar::one();
        let blinder_polynomial = Polynomial(blinder_vec);
        assert_eq!(blinder_polynomial.eval(&self.powers_omega[0]), Scalar::zero());

        PreprocessedInput {
            kzg_set: Kzg10::setup(),
            blinder_polynomial,
            sigma_star,
            qm_x,
            ql_x,
            qr_x,
            qo_x,
            qc_x,
            qs1_x,
            qs2_x,
            qs3_x,
            constraints: self.clone(),
        }
    }
}