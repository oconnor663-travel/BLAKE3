use blake3_guts as guts;
use guts::{CVBytes, CVWords, BLOCK_LEN, CHUNK_LEN};

use core::cmp;
use core::usize;
use rand::prelude::*;

// Interesting input lengths to run tests on.
pub const TEST_CASES: &[usize] = &[
    0,
    1,
    2,
    3,
    4,
    5,
    6,
    7,
    8,
    BLOCK_LEN - 1,
    BLOCK_LEN,
    BLOCK_LEN + 1,
    2 * BLOCK_LEN - 1,
    2 * BLOCK_LEN,
    2 * BLOCK_LEN + 1,
    CHUNK_LEN - 1,
    CHUNK_LEN,
    CHUNK_LEN + 1,
    2 * CHUNK_LEN,
    2 * CHUNK_LEN + 1,
    3 * CHUNK_LEN,
    3 * CHUNK_LEN + 1,
    4 * CHUNK_LEN,
    4 * CHUNK_LEN + 1,
    5 * CHUNK_LEN,
    5 * CHUNK_LEN + 1,
    6 * CHUNK_LEN,
    6 * CHUNK_LEN + 1,
    7 * CHUNK_LEN,
    7 * CHUNK_LEN + 1,
    8 * CHUNK_LEN,
    8 * CHUNK_LEN + 1,
    16 * CHUNK_LEN,  // AVX512's bandwidth
    31 * CHUNK_LEN,  // 16 + 8 + 4 + 2 + 1
    100 * CHUNK_LEN, // subtrees larger than MAX_SIMD_DEGREE chunks
];

pub const TEST_CASES_MAX: usize = 100 * CHUNK_LEN;

// There's a test to make sure these two are equal below.
pub const TEST_KEY: &CVBytes = b"whats the Elvish word for friend";
pub const TEST_KEY_WORDS: &CVWords = &guts::words_from_le_bytes_32(TEST_KEY);

#[test]
fn test_key_bytes_equal_key_words() {
    assert_eq!(TEST_KEY, &guts::le_bytes_from_words_32(TEST_KEY_WORDS),);
}

#[test]
fn test_reference_impl_size() {
    // Because the Rust compiler optimizes struct layout, it's possible that
    // some future version of the compiler will produce a different size. If
    // that happens, we can either disable this test, or test for multiple
    // expected values. For now, the purpose of this test is to make sure we
    // notice if that happens.
    assert_eq!(1880, core::mem::size_of::<reference_impl::Hasher>());
}

pub(crate) fn paint_test_input(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = (i % 251) as u8;
    }
}

#[test]
fn test_compare_reference_impl() {
    const OUT: usize = 303; // more than 64, not a multiple of 4
    let mut input_buf = [0; TEST_CASES_MAX];
    paint_test_input(&mut input_buf);
    for &case in TEST_CASES {
        let input = &input_buf[..case];
        #[cfg(feature = "std")]
        dbg!(case);

        // regular
        {
            let mut reference_hasher = reference_impl::Hasher::new();
            reference_hasher.update(input);
            let mut expected_out = [0; OUT];
            reference_hasher.finalize(&mut expected_out);

            // all at once
            let test_out = crate::hash(input);
            assert_eq!(test_out, expected_out[..32]);
            // incremental
            let mut hasher = crate::Hasher::new();
            hasher.update(input);
            assert_eq!(hasher.finalize(), expected_out[..32]);
            assert_eq!(hasher.finalize(), test_out);
            // incremental (rayon)
            #[cfg(feature = "rayon")]
            {
                let mut hasher = crate::Hasher::new();
                hasher.update_rayon(input);
                assert_eq!(hasher.finalize(), expected_out[..32]);
                assert_eq!(hasher.finalize(), test_out);
            }
            // xof
            let mut extended = [0; OUT];
            hasher.finalize_xof().fill(&mut extended);
            assert_eq!(extended, expected_out);
        }

        // keyed
        {
            let mut reference_hasher = reference_impl::Hasher::new_keyed(TEST_KEY);
            reference_hasher.update(input);
            let mut expected_out = [0; OUT];
            reference_hasher.finalize(&mut expected_out);

            // all at once
            let test_out = crate::keyed_hash(TEST_KEY, input);
            assert_eq!(test_out, expected_out[..32]);
            // incremental
            let mut hasher = crate::Hasher::new_keyed(TEST_KEY);
            hasher.update(input);
            assert_eq!(hasher.finalize(), expected_out[..32]);
            assert_eq!(hasher.finalize(), test_out);
            // incremental (rayon)
            #[cfg(feature = "rayon")]
            {
                let mut hasher = crate::Hasher::new_keyed(TEST_KEY);
                hasher.update_rayon(input);
                assert_eq!(hasher.finalize(), expected_out[..32]);
                assert_eq!(hasher.finalize(), test_out);
            }
            // xof
            let mut extended = [0; OUT];
            hasher.finalize_xof().fill(&mut extended);
            assert_eq!(extended, expected_out);
        }

        // derive_key
        {
            let context = "BLAKE3 2019-12-27 16:13:59 example context (not the test vector one)";
            let mut reference_hasher = reference_impl::Hasher::new_derive_key(context);
            reference_hasher.update(input);
            let mut expected_out = [0; OUT];
            reference_hasher.finalize(&mut expected_out);

            // all at once
            let test_out = crate::derive_key(context, input);
            assert_eq!(test_out, expected_out[..32]);
            // incremental
            let mut hasher = crate::Hasher::new_derive_key(context);
            hasher.update(input);
            assert_eq!(hasher.finalize(), expected_out[..32]);
            assert_eq!(hasher.finalize(), test_out[..32]);
            // incremental (rayon)
            #[cfg(feature = "rayon")]
            {
                let mut hasher = crate::Hasher::new_derive_key(context);
                hasher.update_rayon(input);
                assert_eq!(hasher.finalize(), expected_out[..32]);
                assert_eq!(hasher.finalize(), test_out[..32]);
            }
            // xof
            let mut extended = [0; OUT];
            hasher.finalize_xof().fill(&mut extended);
            assert_eq!(extended, expected_out);
        }
    }
}

fn reference_hash(input: &[u8]) -> crate::Hash {
    let mut hasher = reference_impl::Hasher::new();
    hasher.update(input);
    let mut bytes = [0; 32];
    hasher.finalize(&mut bytes);
    bytes.into()
}

#[test]
fn test_compare_update_multiple() {
    // Don't use all the long test cases here, since that's unnecessarily slow
    // in debug mode.
    let mut short_test_cases = TEST_CASES;
    while *short_test_cases.last().unwrap() > 4 * CHUNK_LEN {
        short_test_cases = &short_test_cases[..short_test_cases.len() - 1];
    }
    assert_eq!(*short_test_cases.last().unwrap(), 4 * CHUNK_LEN);

    let mut input_buf = [0; 2 * TEST_CASES_MAX];
    paint_test_input(&mut input_buf);

    for &first_update in short_test_cases {
        #[cfg(feature = "std")]
        dbg!(first_update);
        let first_input = &input_buf[..first_update];
        let mut test_hasher = crate::Hasher::new();
        test_hasher.update(first_input);

        for &second_update in short_test_cases {
            #[cfg(feature = "std")]
            dbg!(second_update);
            let second_input = &input_buf[first_update..][..second_update];
            let total_input = &input_buf[..first_update + second_update];

            // Clone the hasher with first_update bytes already written, so
            // that the next iteration can reuse it.
            let mut test_hasher = test_hasher.clone();
            test_hasher.update(second_input);
            let expected = reference_hash(total_input);
            assert_eq!(expected, test_hasher.finalize());
        }
    }
}

#[test]
fn test_fuzz_hasher() {
    const INPUT_MAX: usize = 4 * CHUNK_LEN;
    let mut input_buf = [0; 3 * INPUT_MAX];
    paint_test_input(&mut input_buf);

    // Don't do too many iterations in debug mode, to keep the tests under a second or so. CI
    // should run tests in release mode also.
    // TODO: Provide an environment variable for specifying a larger number of fuzz iterations?
    let num_tests = if cfg!(debug_assertions) { 100 } else { 10_000 };

    // Use a fixed RNG seed for reproducibility.
    let mut rng = rand_chacha::ChaCha8Rng::from_seed([1; 32]);
    for _num_test in 0..num_tests {
        #[cfg(feature = "std")]
        dbg!(_num_test);
        let mut hasher = crate::Hasher::new();
        let mut total_input = 0;
        // For each test, write 3 inputs of random length.
        for _ in 0..3 {
            let input_len = rng.gen_range(0..(INPUT_MAX + 1));
            #[cfg(feature = "std")]
            dbg!(input_len);
            let input = &input_buf[total_input..][..input_len];
            hasher.update(input);
            total_input += input_len;
        }
        let expected = reference_hash(&input_buf[..total_input]);
        assert_eq!(expected, hasher.finalize());
    }
}

#[test]
fn test_xof_seek() {
    let mut out = [0; 533];
    let mut hasher = crate::Hasher::new();
    hasher.update(b"foo");
    hasher.finalize_xof().fill(&mut out);
    assert_eq!(hasher.finalize().as_bytes(), &out[0..32]);

    let mut reader = hasher.finalize_xof();
    reader.set_position(303);
    let mut out2 = [0; 102];
    reader.fill(&mut out2);
    assert_eq!(&out[303..][..102], &out2[..]);

    #[cfg(feature = "std")]
    {
        use std::io::prelude::*;
        let mut reader = hasher.finalize_xof();
        reader.seek(std::io::SeekFrom::Start(303)).unwrap();
        let mut out3 = Vec::new();
        reader.by_ref().take(102).read_to_end(&mut out3).unwrap();
        assert_eq!(&out[303..][..102], &out3[..]);

        assert_eq!(
            reader.seek(std::io::SeekFrom::Current(0)).unwrap(),
            303 + 102
        );
        reader.seek(std::io::SeekFrom::Current(-5)).unwrap();
        assert_eq!(
            reader.seek(std::io::SeekFrom::Current(0)).unwrap(),
            303 + 102 - 5
        );
        let mut out4 = [0; 17];
        assert_eq!(reader.read(&mut out4).unwrap(), 17);
        assert_eq!(&out[303 + 102 - 5..][..17], &out4[..]);
        assert_eq!(
            reader.seek(std::io::SeekFrom::Current(0)).unwrap(),
            303 + 102 - 5 + 17
        );
        assert!(reader.seek(std::io::SeekFrom::End(0)).is_err());
        assert!(reader.seek(std::io::SeekFrom::Current(-1000)).is_err());
    }
}

#[test]
fn test_xof_xor() {
    for step in [32, 63, 64, 128, 303] {
        #[cfg(feature = "std")]
        dbg!(step);
        let mut ref_hasher = reference_impl::Hasher::new();
        ref_hasher.update(b"foo");
        let mut ref_output = [0u8; 1000];
        ref_hasher.finalize(&mut ref_output);

        let mut hasher = crate::Hasher::new();
        hasher.update(b"foo");
        let mut reader = hasher.finalize_xof();

        let mut test_output = [0u8; 1000];
        for chunk in test_output.chunks_mut(step) {
            reader.fill(chunk);
        }
        assert_eq!(ref_output, test_output);
        // Xor'ing the same output should zero the buffer.
        reader.set_position(0);
        for chunk in test_output.chunks_mut(step) {
            reader.fill_xor(chunk);
        }
        assert_eq!([0u8; 1000], test_output);
        // Xor'ing the same output again should reproduce the original.
        reader.set_position(0);
        for chunk in test_output.chunks_mut(step) {
            reader.fill_xor(chunk);
        }
        assert_eq!(ref_output, test_output);

        // Repeat the same test but starting at offset 500.
        reader.set_position(500);
        for chunk in test_output[..500].chunks_mut(step) {
            reader.fill(chunk);
        }
        assert_eq!(ref_output[500..], test_output[..500]);
        reader.set_position(500);
        for chunk in test_output[..500].chunks_mut(step) {
            reader.fill_xor(chunk);
        }
        assert_eq!([0u8; 500], test_output[..500]);
        reader.set_position(500);
        for chunk in test_output[..500].chunks_mut(step) {
            reader.fill_xor(chunk);
        }
        assert_eq!(ref_output[500..], test_output[..500]);
    }
}

#[test]
#[cfg(feature = "std")]
fn test_fuzz_xof() {
    // Use a fixed RNG seed for reproducibility.
    let mut rng = rand_chacha::ChaCha8Rng::from_seed([99; 32]);
    let random_key: [u8; 32] = rng.gen();

    let possible_seeks = [-64i64, -63 - 1, 0, 1, 63, 64, 127, 128, 129];

    const MAX_LEN: usize = 1100;
    let possible_lengths = [0usize, 1, 63, 64, 65, 128, 256, 512, 1024, MAX_LEN];
    assert!(possible_lengths.into_iter().all(|x| x <= MAX_LEN));

    let mut xof_output = crate::Hasher::new_keyed(&random_key).finalize_xof();
    let mut xof_xor_output = crate::Hasher::new_keyed(&random_key).finalize_xof();

    // Don't do too many iterations in debug mode, to keep the tests under a second or so. CI
    // should run tests in release mode also.
    // TODO: Provide an environment variable for specifying a larger number of fuzz iterations?
    let num_tests = if cfg!(debug_assertions) {
        1_000
    } else {
        100_000
    };

    let mut position = 0;
    let mut ref_output = Vec::new();
    for test_i in 0..num_tests {
        eprintln!("--- test {test_i} ---");
        // Do a random relative seek maybe. Could be zero.
        let relative_seek: i64 = *possible_seeks.choose(&mut rng).unwrap();
        dbg!(relative_seek);
        if relative_seek != 0 {
            let new_position = position as i64 + relative_seek;
            if 0 <= new_position && new_position <= MAX_LEN as i64 {
                position = new_position as u64;
            } else {
                position = 0;
            }
            assert_eq!(xof_output.position(), xof_xor_output.position());
            xof_output.set_position(position as u64);
            xof_xor_output.set_position(position as u64);
        }
        dbg!(position);

        // Generate a random number of output bytes. If the amount of output we've gotten from the
        // reference_impl isn't enough, double it.
        let len: usize = *possible_lengths.choose(&mut rng).unwrap();
        dbg!(len);
        if position as usize + len > ref_output.len() {
            let new_len = cmp::max(MAX_LEN, 2 * ref_output.len());
            ref_output = vec![0u8; new_len];
            eprintln!("grow reference output length to {}", ref_output.len());
            let ref_hasher = reference_impl::Hasher::new_keyed(&random_key);
            ref_hasher.finalize(&mut ref_output);
        }
        let mut buf = [0u8; MAX_LEN];
        xof_output.fill(&mut buf[..len]);
        assert_eq!(ref_output[position as usize..][..len], buf[..len]);
        assert_eq!([0u8; MAX_LEN][..MAX_LEN - len], buf[len..]);

        // Xor over the output with a random byte value, and then confirm that xof_xor() recovers
        // that value.
        let random_byte: u8 = rng.gen();
        dbg!(random_byte);
        for i in 0..len {
            buf[i] ^= random_byte;
        }
        xof_xor_output.fill_xor(&mut buf[..len]);
        assert_eq!([random_byte; MAX_LEN][..len], buf[..len]);
        assert_eq!([0u8; MAX_LEN][..MAX_LEN - len], buf[len..]);

        position += len as u64;
    }
}

#[test]
fn test_msg_schedule_permutation() {
    let permutation = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

    let mut generated = [[0; 16]; 7];
    generated[0] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

    for round in 1..7 {
        for i in 0..16 {
            generated[round][i] = generated[round - 1][permutation[i]];
        }
    }

    assert_eq!(generated, guts::MSG_SCHEDULE);
}

#[test]
fn test_reset() {
    let mut hasher = crate::Hasher::new();
    hasher.update(&[42; 3 * CHUNK_LEN + 7]);
    hasher.reset();
    hasher.update(&[42; CHUNK_LEN + 3]);
    assert_eq!(hasher.finalize(), crate::hash(&[42; CHUNK_LEN + 3]));

    let key = &[99; crate::KEY_LEN];
    let mut keyed_hasher = crate::Hasher::new_keyed(key);
    keyed_hasher.update(&[42; 3 * CHUNK_LEN + 7]);
    keyed_hasher.reset();
    keyed_hasher.update(&[42; CHUNK_LEN + 3]);
    assert_eq!(
        keyed_hasher.finalize(),
        crate::keyed_hash(key, &[42; CHUNK_LEN + 3]),
    );

    let context = "BLAKE3 2020-02-12 10:20:58 reset test";
    let mut kdf = crate::Hasher::new_derive_key(context);
    kdf.update(&[42; 3 * CHUNK_LEN + 7]);
    kdf.reset();
    kdf.update(&[42; CHUNK_LEN + 3]);
    let expected = crate::derive_key(context, &[42; CHUNK_LEN + 3]);
    assert_eq!(kdf.finalize(), expected);
}

#[test]
fn test_hex_encoding_decoding() {
    let digest_str = "04e0bb39f30b1a3feb89f536c93be15055482df748674b00d26e5a75777702e9";
    let mut hasher = crate::Hasher::new();
    hasher.update(b"foo");
    let digest = hasher.finalize();
    assert_eq!(digest.to_hex().as_str(), digest_str);
    #[cfg(feature = "std")]
    assert_eq!(digest.to_string(), digest_str);

    // Test round trip
    let digest = crate::Hash::from_hex(digest_str).unwrap();
    assert_eq!(digest.to_hex().as_str(), digest_str);

    // Test uppercase
    let digest = crate::Hash::from_hex(digest_str.to_uppercase()).unwrap();
    assert_eq!(digest.to_hex().as_str(), digest_str);

    // Test string parsing via FromStr
    let digest: crate::Hash = digest_str.parse().unwrap();
    assert_eq!(digest.to_hex().as_str(), digest_str);

    // Test errors
    let bad_len = "04e0bb39f30b1";
    let _result = crate::Hash::from_hex(bad_len).unwrap_err();
    #[cfg(feature = "std")]
    assert_eq!(_result.to_string(), "expected 64 hex bytes, received 13");

    let bad_char = "Z4e0bb39f30b1a3feb89f536c93be15055482df748674b00d26e5a75777702e9";
    let _result = crate::Hash::from_hex(bad_char).unwrap_err();
    #[cfg(feature = "std")]
    assert_eq!(_result.to_string(), "invalid hex character: 'Z'");

    let _result = crate::Hash::from_hex([128; 64]).unwrap_err();
    #[cfg(feature = "std")]
    assert_eq!(_result.to_string(), "invalid hex character: 0x80");
}

// This test is a mimized failure case for the Windows SSE2 bug described in
// https://github.com/BLAKE3-team/BLAKE3/issues/206.
//
// Before that issue was fixed, this test would fail on Windows in the following configuration:
//
//     cargo test --features=no_avx512,no_avx2,no_sse41 --release
//
// Bugs like this one (stomping on a caller's register) are very sensitive to the details of
// surrounding code, so it's not especially likely that this test will catch another bug (or even
// the same bug) in the future. Still, there's no harm in keeping it.
#[test]
fn test_issue_206_windows_sse2() {
    // This stupid loop has to be here to trigger the bug. I don't know why.
    for _ in &[0] {
        // The length 65 (two blocks) is significant. It doesn't repro with 64 (one block). It also
        // doesn't repro with an all-zero input.
        let input = &[0xff; 65];
        let expected_hash = [
            183, 235, 50, 217, 156, 24, 190, 219, 2, 216, 176, 255, 224, 53, 28, 95, 57, 148, 179,
            245, 162, 90, 37, 121, 0, 142, 219, 62, 234, 204, 225, 161,
        ];

        // This throwaway call has to be here to trigger the bug.
        crate::Hasher::new().update(input);

        // This assert fails when the bug is triggered.
        assert_eq!(crate::Hasher::new().update(input).finalize(), expected_hash);
    }
}

#[test]
fn test_hash_conversions() {
    let bytes1 = [42; 32];
    let hash1: crate::Hash = bytes1.into();
    let bytes2: [u8; 32] = hash1.into();
    assert_eq!(bytes1, bytes2);

    let bytes3 = *hash1.as_bytes();
    assert_eq!(bytes1, bytes3);

    let hash2 = crate::Hash::from_bytes(bytes1);
    assert_eq!(hash1, hash2);

    let hex = hash1.to_hex();
    let hash3 = crate::Hash::from_hex(hex.as_bytes()).unwrap();
    assert_eq!(hash1, hash3);
}

#[test]
const fn test_hash_const_conversions() {
    let bytes = [42; 32];
    let hash = crate::Hash::from_bytes(bytes);
    _ = hash.as_bytes();
}

#[cfg(feature = "zeroize")]
#[test]
fn test_zeroize() {
    use zeroize::Zeroize;

    let mut hash = crate::Hash([42; 32]);
    hash.zeroize();
    assert_eq!(hash.0, [0u8; 32]);

    let mut hasher = crate::Hasher {
        chunk_state: crate::ChunkState {
            cv: [42; 32],
            chunk_counter: 42,
            buf: [42; 64],
            buf_len: 42,
            blocks_compressed: 42,
            flags: 42,
        },
        key: [42; 32],
        cv_stack: [[42; 32]; { crate::MAX_DEPTH + 1 }].into(),
    };
    hasher.zeroize();
    assert_eq!(hasher.chunk_state.cv, [0; 32]);
    assert_eq!(hasher.chunk_state.chunk_counter, 0);
    assert_eq!(hasher.chunk_state.buf, [0; 64]);
    assert_eq!(hasher.chunk_state.buf_len, 0);
    assert_eq!(hasher.chunk_state.blocks_compressed, 0);
    assert_eq!(hasher.chunk_state.flags, 0);
    assert_eq!(hasher.key, [0; 32]);
    assert_eq!(&*hasher.cv_stack, &[[0u8; 32]; 0]);

    let mut output_reader = crate::OutputReader {
        inner: crate::Output {
            input_chaining_value: [42; 32],
            block: [42; 64],
            counter: 42,
            block_len: 42,
            flags: 42,
        },
        position_within_block: 42,
    };

    output_reader.zeroize();
    assert_eq!(output_reader.inner.input_chaining_value, [0; 32]);
    assert_eq!(output_reader.inner.block, [0; 64]);
    assert_eq!(output_reader.inner.counter, 0);
    assert_eq!(output_reader.inner.block_len, 0);
    assert_eq!(output_reader.inner.flags, 0);
    assert_eq!(output_reader.position_within_block, 0);
}

