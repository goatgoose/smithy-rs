/*
 *  Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 *  SPDX-License-Identifier: Apache-2.0
 */

use crate::endpoint_lib::diagnostic::DiagnosticCollector;

/// substring of `input`
///
/// > Note: this function only operates on ASCII input. If the input contains non-ASCII characters,
/// > `None` will be returned.
///
/// - When `reverse` is false, indexes are evaluated from the beginning of the string
/// - When `reverse` is true, indexes are evaluated from the end of the string (however, the result
///   will still be "forwards" and `start` MUST be less than `end`.
pub(crate) fn substring<'a, 'b>(
    input: &'a str,
    start: usize,
    stop: usize,
    reverse: bool,
    e: &'b mut DiagnosticCollector,
) -> Option<&'a str> {
    if start >= stop {
        e.capture(Err("start > stop"))?;
    }
    if !input.is_ascii() {
        e.capture(Err("the input to substring was not ascii"))?;
    }
    if input.len() < stop {
        e.capture(Err("the input was too short"))?;
    }
    let (effective_start, effective_stop) = if !reverse {
        (start, stop)
    } else {
        (input.len() - stop, input.len() - start)
    };
    Some(&input[effective_start..effective_stop])
}

#[cfg(all(test, feature = "gated-tests"))]
mod test {
    use super::*;
    use proptest::proptest;

    #[test]
    fn substring_forwards() {
        assert_eq!(
            substring("hello", 0, 2, false, &mut DiagnosticCollector::new()),
            Some("he")
        );
        assert_eq!(
            substring("hello", 0, 0, false, &mut DiagnosticCollector::new()),
            None
        );
        assert_eq!(
            substring("hello", 0, 5, false, &mut DiagnosticCollector::new()),
            Some("hello")
        );
        assert_eq!(
            substring("hello", 0, 6, false, &mut DiagnosticCollector::new()),
            None
        );
    }
    fn substring_backwards() {
        assert_eq!(
            substring("hello", 0, 2, true, &mut DiagnosticCollector::new()),
            Some("lo")
        );
        assert_eq!(
            substring("hello", 0, 0, true, &mut DiagnosticCollector::new()),
            None
        );
        assert_eq!(
            substring("hello", 0, 5, true, &mut DiagnosticCollector::new()),
            Some("hello")
        )
    }

    // substring doesn't support unicode, it always returns none
    #[test]
    fn substring_unicode() {
        let mut collector = DiagnosticCollector::new();
        assert_eq!(substring("a🐱b", 0, 2, false, &mut collector), None);
        assert_eq!(
            format!(
                "{}",
                collector
                    .take_last_error()
                    .expect("last error should be set")
            ),
            "the input to substring was not ascii"
        );
    }

    use proptest::prelude::*;
    proptest! {
        #[test]
        fn substring_no_panics(s in any::<String>(), start in 0..100usize, stop in 0..100usize, reverse in proptest::bool::ANY) {
            substring(&s, start, stop, reverse, &mut DiagnosticCollector::new());
        }

        #[test]
        fn substring_correct_length(s in r#"[\x00-\xFF]*"#, start in 0..10usize, stop in 0..10usize, reverse in proptest::bool::ANY) {
            prop_assume!(start < s.len());
            prop_assume!(stop < s.len());
            prop_assume!(start < stop);
            if let Some(result) = substring(&s, start, stop, reverse, &mut DiagnosticCollector::new()) {
                assert_eq!(result.len(), stop - start);
            }

        }
    }
}