//! Anemoi constants for BN254 scalar field (Fr).
//!
//! Round constants are adapted from the official anemoi-rust implementation:
//! https://github.com/anemoi-hash/anemoi-rust
//!
//! Note: anemoi-rust uses the base field Fq, but we use the scalar field Fr.
//! INV_ALPHA is computed specifically for Fr.

use ark_bn254::Fr;
use ark_ff::{Field, MontFp};
use num_bigint::BigUint;
use num_traits::{One, Zero};
use std::sync::OnceLock;

/// Number of rounds for 128-bit security with 2 cells (state width 2).
pub const NUM_ROUNDS: usize = 21;

/// S-box exponent alpha = 5.
pub const ALPHA: u32 = 5;

/// Cached INV_ALPHA for Fr (computed once on first use).
static INV_ALPHA_CACHE: OnceLock<[u64; 4]> = OnceLock::new();

/// Compute modular inverse using extended Euclidean algorithm.
fn mod_inverse(a: &BigUint, m: &BigUint) -> Option<BigUint> {
    use num_bigint::BigInt;
    use num_traits::Signed;

    if a.is_zero() || m.is_zero() {
        return None;
    }

    let a = BigInt::from(a.clone());
    let m = BigInt::from(m.clone());

    let (mut old_r, mut r) = (m.clone(), a);
    let (mut old_s, mut s) = (BigInt::zero(), BigInt::one());

    while !r.is_zero() {
        let q = &old_r / &r;
        let temp_r = &old_r - &q * &r;
        old_r = r;
        r = temp_r;

        let temp_s = &old_s - &q * &s;
        old_s = s;
        s = temp_s;
    }

    if old_r != BigInt::one() {
        return None;
    }

    let result = if old_s.is_negative() {
        old_s + m
    } else {
        old_s
    };

    Some(result.to_biguint().unwrap())
}

/// Get INV_ALPHA for Fr field.
/// Computed as 5^(-1) mod (r-1) where r is the scalar field order.
fn get_inv_alpha() -> [u64; 4] {
    *INV_ALPHA_CACHE.get_or_init(|| {
        // Fr field order (BN254 scalar field)
        let r = BigUint::parse_bytes(
            b"21888242871839275222246405745257275088548364400416034343698204186575808495617",
            10,
        ).unwrap();
        let r_minus_1 = &r - BigUint::one();

        // Compute 5^(-1) mod (r-1)
        let alpha = BigUint::from(5u64);
        let inv_alpha = mod_inverse(&alpha, &r_minus_1)
            .expect("5 should be invertible mod (r-1)");

        // Convert to [u64; 4] little-endian
        let bytes = inv_alpha.to_bytes_le();
        let mut limbs = [0u64; 4];
        for (i, chunk) in bytes.chunks(8).enumerate() {
            if i >= 4 {
                break;
            }
            let mut arr = [0u8; 8];
            arr[..chunk.len()].copy_from_slice(chunk);
            limbs[i] = u64::from_le_bytes(arr);
        }

        limbs
    })
}

/// Beta constant for the Flystel S-box.
/// From anemoi-rust: BETA = 3
pub const BETA: u32 = 3;

/// Generator for multiplication (equals BETA).
pub const GENERATOR: u32 = BETA;

/// Delta constant for the Flystel S-box.
/// From anemoi-rust: MontFp!("14592161914559516814830937163504850059130874104865215775126025263096817472389")
pub const DELTA: Fr = MontFp!("14592161914559516814830937163504850059130874104865215775126025263096817472389");

/// Additive round constants C (applied to x).
/// From anemoi-rust: src/bn_254/anemoi_2_1/round_constants.rs
pub const ARK_C: [Fr; NUM_ROUNDS] = [
    MontFp!("35"),
    MontFp!("6295152911226189444253866884160054349659895788778004607372136398144290407549"),
    MontFp!("21393954857409224592174814995535004102448434148992032619414656579779568689397"),
    MontFp!("2634462417634249118966770176700483350170452174029521955965196796017381401956"),
    MontFp!("15295217319366023946214435818896821054815840546749644406149416251837787741809"),
    MontFp!("4897740601044642812417701466299626155416058382941267461556139867957028325237"),
    MontFp!("5767392289724148929423693389240961480732135563465616880868228443921968183406"),
    MontFp!("5317720115970311700954487800253539185353124589203406383093606299177363378965"),
    MontFp!("7178683072420401867285583491757519765705805053793732990705659310878562779654"),
    MontFp!("2383742814212199693233039241857741773656918170662619722497679382455110461025"),
    MontFp!("21825004911469755557450894072491638633707168470327514863312230079098931792558"),
    MontFp!("21450247751664710150210987593785515492243624894603811466130600962437485349126"),
    MontFp!("19967544120700448884148022659483814768484183439673469569018785205921257830429"),
    MontFp!("18386115521729639027684330339118321986942103783973318486465519864576242574862"),
    MontFp!("5222876701568318110831681508842165303986917174375317349375140545041481764952"),
    MontFp!("9231355910670255050575106888255556928562119461803893300039002639706141560902"),
    MontFp!("3344680549611944996884536409173096884900644175187172134191926446369071942939"),
    MontFp!("11551048920232318848453944681589812526402907579066154228095000866622742487951"),
    MontFp!("5770545679727133125848730864790597325286409961887011992575465370931540893204"),
    MontFp!("15175705884786220884440327623359026942802564712286805338564472872304437600541"),
    MontFp!("6799200415199776888080914636284593740227340143560640150474634024748340141261"),
];

/// Additive round constants D (applied to y).
/// From anemoi-rust: src/bn_254/anemoi_2_1/round_constants.rs
pub const ARK_D: [Fr; NUM_ROUNDS] = [
    MontFp!("14592161914559516814830937163504850059130874104865215775126025263096817472424"),
    MontFp!("14886328771476750059923328797989224350922636562469070718728625674557930694836"),
    MontFp!("19638893102775796275277235225427456246362977103230584685008442902017466767651"),
    MontFp!("1787913011046289936440351532682066056745691701271258002538193117792994935178"),
    MontFp!("6540948924772883830594916229804018068216315941466299076736273903153878004006"),
    MontFp!("9293040363042818517653611216357198985940406678571067812961313240862902841192"),
    MontFp!("20658070995971503522675594453880534996148127144823809317500067332141426153766"),
    MontFp!("16109693438097508500290383924735749637046547901319402337827102095664399270999"),
    MontFp!("18656175406244756418120480628945495368630959223459057438730888996407700600910"),
    MontFp!("12502826677069997362424575811268729535889132287156664980829561599509861883271"),
    MontFp!("16259278398410221045718709682000965909307495157015098081392577671769584472840"),
    MontFp!("5279985636464436227957162207029856553960115578931399779648839709840928288577"),
    MontFp!("10994211793969826214874362360644675501561401171942888501268420197333659245365"),
    MontFp!("19252379911722914953209805612883583535327217956088185615886897738157450269129"),
    MontFp!("10424568097150255112528687259845077716279978527111882303821008504816615539159"),
    MontFp!("1621759403088675376655742261730998000163691586395589568493875128794000199672"),
    MontFp!("304985833190546383798805007650597076598916014823709920501772316800584520997"),
    MontFp!("16726531536278306043199462544871150636690858999103640779036788472646171850938"),
    MontFp!("1474528789338045826667987129215105621796134307107476035177634670013775973228"),
    MontFp!("20428839980229229806491630450745076737048918250928120281030617209856216531357"),
    MontFp!("21239294466999674498710702227798383396644192544233111184864865815113875456424"),
];

/// Compute x^(1/alpha) = x^INV_ALPHA in the field.
/// This is the inverse S-box operation.
#[inline]
pub fn exp_inv_alpha(x: Fr) -> Fr {
    // Use the precomputed INV_ALPHA for Fr
    x.pow(get_inv_alpha())
}

/// Compute x^alpha = x^5 in the field.
/// This is the forward S-box operation.
#[inline]
pub fn exp_alpha(x: Fr) -> Fr {
    let x2 = x.square();
    let x4 = x2.square();
    x4 * x
}

/// Multiply by generator (optimized for g=3).
#[inline]
pub fn mul_by_generator(x: Fr) -> Fr {
    x.double() + x
}

/// Get beta as field element.
#[inline]
pub fn beta() -> Fr {
    Fr::from(BETA)
}

/// Get delta constant.
#[inline]
pub fn delta() -> Fr {
    DELTA
}

#[cfg(test)]
mod const_tests {
    use super::*;

    #[test]
    fn test_inv_alpha_correct() {
        // Verify that x^(1/5)^5 = x for various test values
        for i in 1u64..100 {
            let x = Fr::from(i);
            let x_inv = exp_inv_alpha(x);
            let x_back = exp_alpha(x_inv);
            assert_eq!(x, x_back, "x^(1/5)^5 should equal x for x={}", i);
        }
    }

    #[test]
    fn test_generator_multiplication() {
        let x = Fr::from(42u64);
        let expected = Fr::from(3u64) * x;
        let actual = mul_by_generator(x);
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_constants_not_zero() {
        // Sanity check that constants loaded correctly
        let inv_alpha = get_inv_alpha();
        assert!(inv_alpha.iter().any(|&x| x != 0), "INV_ALPHA should be non-zero");
        assert_ne!(DELTA, Fr::from(0u64));
        assert_ne!(ARK_C[0], Fr::from(0u64));
        assert_ne!(ARK_D[0], Fr::from(0u64));
    }
}
