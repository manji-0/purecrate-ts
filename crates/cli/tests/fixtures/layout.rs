// Long names that make each construct break, so the layout of what oxfmt
// would break differently is checked (scripts/verify.sh runs oxfmt --check
// over every fixture's output). Each function is one shape.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorizationFailureReason {
    AuthorizationCodeAlreadyRedeemedByAnotherClient,
    RedirectLocationDoesNotMatchTheRegistration,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VeryLongConfigurationRecord {
    pub maximum_number_of_password_attempts: u32,
    pub maximum_number_of_one_time_code_attempts: u32,
    pub minimum_length_of_the_generated_identifier: u32,
    pub nested_outcome_of_the_last_lookup: Result<Option<Result<u32, u32>>, u32>,
}

pub fn every_limit_is_positive_and_ordered(config: &VeryLongConfigurationRecord) -> bool {
    config.maximum_number_of_password_attempts > 0 || config.maximum_number_of_one_time_code_attempts > 0 || config.minimum_length_of_the_generated_identifier > 0
}

pub fn all_limits_hold_together(config: &VeryLongConfigurationRecord, extra: u32) -> bool {
    let holds = config.maximum_number_of_password_attempts > extra && config.maximum_number_of_one_time_code_attempts > extra && config.minimum_length_of_the_generated_identifier > extra;
    holds
}

fn compute_the_total_number_of_attempts_allowed(config: &VeryLongConfigurationRecord) -> u32 {
    config.maximum_number_of_password_attempts + config.maximum_number_of_one_time_code_attempts
}

pub fn assigned_from_a_long_call(config: &VeryLongConfigurationRecord) -> u32 {
    let total_number_of_attempts_allowed_for_this_configuration: u32 = compute_the_total_number_of_attempts_allowed(config);
    total_number_of_attempts_allowed_for_this_configuration + 1
}

pub fn count_down_while_both_hold(config: &VeryLongConfigurationRecord, remaining_attempt_budget: u32) -> u32 {
    let mut remaining_attempt_budget = remaining_attempt_budget;
    while remaining_attempt_budget > config.minimum_length_of_the_generated_identifier && remaining_attempt_budget > config.maximum_number_of_password_attempts {
        remaining_attempt_budget -= 1;
    }
    remaining_attempt_budget
}

pub fn classify_the_attempt_count(config: &VeryLongConfigurationRecord, observed_attempt_count: u32) -> u32 {
    if observed_attempt_count > config.maximum_number_of_password_attempts {
        1
    } else if observed_attempt_count > config.maximum_number_of_one_time_code_attempts && observed_attempt_count > config.minimum_length_of_the_generated_identifier {
        2
    } else {
        3
    }
}

fn is_an_unreserved_character_in_the_identifier(byte_value: u8, allow_tilde_character: bool) -> bool {
    matches!(byte_value, b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z') || (allow_tilde_character && byte_value == b'~')
}

pub fn every_character_is_unreserved(identifier_text: &str, allow_tilde_character: bool) -> bool {
    identifier_text.bytes().all(|b| is_an_unreserved_character_in_the_identifier(b, allow_tilde_character))
}

pub fn deeply_nested_failure(a: u32, b: u32, c: u32, d: u32) -> Result<u32, AuthorizationFailureReason> {
    if a > 0 {
        if b > 0 {
            if c > 0 {
                if d > 0 {
                    if a > b {
                        if b > c {
                            return Err(AuthorizationFailureReason::AuthorizationCodeAlreadyRedeemedByAnotherClient);
                        }
                    }
                }
            }
        }
    }
    Ok(a)
}

pub fn sniff(value: u32) -> u32 {
    value + 1
}

pub fn diff_of_the_two_measured_values(first_measured_value: u32, second_measured_value: u32) -> u32 {
    sniff(first_measured_value) + sniff(second_measured_value) + sniff(first_measured_value + second_measured_value)
}

pub fn ratio(a: f64, b: f64, c: f64) -> f64 {
    a * b / c
}

pub fn remainder_sum(a: f64, b: f64, c: f64) -> f64 {
    a + b % c
}

pub fn japanese_message_is_short_enough(code: u32) -> &'static str {
    if code == 0 { "金額は正の数でなければなりません。もう一度入力してください。" } else { "ok" }
}

pub fn argument_is_a_block(first_argument_value: u32, second_argument_value: Option<u32>) -> u32 {
    compute_the_total_number_of_attempts_allowed(&VeryLongConfigurationRecord {
        maximum_number_of_password_attempts: first_argument_value,
        maximum_number_of_one_time_code_attempts: match second_argument_value { Some(v) => { let w = v * 2; w + 1 } None => { let z = first_argument_value * 3; z - 1 } },
        minimum_length_of_the_generated_identifier: 0,
        nested_outcome_of_the_last_lookup: Ok(None),
    })
}

fn is_upper_case_letter(b: u8) -> bool {
    matches!(b, b'A'..=b'Z')
}

fn is_decimal_digit(b: u8) -> bool {
    matches!(b, b'0'..=b'9')
}

pub fn either_class_at_the_position(text_bytes: &[u8], position_in_text: usize) -> bool {
    is_upper_case_letter(text_bytes[position_in_text]) || is_decimal_digit(text_bytes[position_in_text]) || text_bytes[position_in_text] == b'_'
}

pub fn all_three_versions_agree(first_major: u64, second_major: u64, first_minor: u64, second_minor: u64) -> bool {
    first_major == second_major && first_minor == second_minor && first_major + first_minor == second_major + second_minor
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcessingStateOfTheRequest {
    WaitingForTheFirstSubmission { attempts_so_far: u32 },
    CompletedWithAnAcceptedOutcome { accepted_value: u32, attempts_so_far: u32 },
}

pub fn advance_the_processing_state(state: ProcessingStateOfTheRequest, submitted_value: u32) -> ProcessingStateOfTheRequest {
    match state {
        ProcessingStateOfTheRequest::WaitingForTheFirstSubmission { attempts_so_far } => ProcessingStateOfTheRequest::CompletedWithAnAcceptedOutcome { accepted_value: submitted_value, attempts_so_far: attempts_so_far + 1 },
        ProcessingStateOfTheRequest::CompletedWithAnAcceptedOutcome { accepted_value, attempts_so_far } => ProcessingStateOfTheRequest::CompletedWithAnAcceptedOutcome { accepted_value: accepted_value + submitted_value, attempts_so_far },
    }
}

pub fn first_or_fallback_value(maybe_first_value: Option<u32>, fallback_value_when_missing: u32) -> u32 {
    maybe_first_value.map(|value_present| value_present * 2 + fallback_value_when_missing).unwrap_or(fallback_value_when_missing)
}

/// A last-argument arrow whose body is `||`: oxfmt opens every argument.
pub fn is_known_prompt_value_list(prompt_value_list: &str) -> bool {
    prompt_value_list.split(' ').all(|t| t == "" || t == "none" || t == "login" || t == "consent" || t == "select_account")
}

/// A long `&&` chain bound by `let`: broken after `=`, one operand a line.
pub fn limits_bound_together(config: &VeryLongConfigurationRecord, extra_allowance: u32) -> u32 {
    let within_every_limit = config.maximum_number_of_password_attempts > extra_allowance
        && config.maximum_number_of_one_time_code_attempts > extra_allowance
        && config.minimum_length_of_the_generated_identifier < extra_allowance;
    if within_every_limit {
        return 1;
    }
    count_the_limits_that_hold(within_every_limit)
}

fn count_the_limits_that_hold(every_limit_holds: bool) -> u32 {
    if every_limit_holds {
        1
    } else {
        0
    }
}

/// A long `&&` chain as a call's one argument: its operands after the first
/// one indent in.
pub fn both_limits_exceed(config: &VeryLongConfigurationRecord, extra_allowance: u32) -> u32 {
    count_the_limits_that_hold(
        config.maximum_number_of_password_attempts > extra_allowance
            && config.maximum_number_of_one_time_code_attempts > extra_allowance,
    )
}

fn split_the_configuration_record(
    config: &VeryLongConfigurationRecord,
) -> (VeryLongConfigurationRecord, AuthorizationFailureReason) {
    (config.clone(), AuthorizationFailureReason::AuthorizationCodeAlreadyRedeemedByAnotherClient)
}

/// A tuple from a call taken apart needs no annotation, so the line fits.
pub fn reason_after_the_split(config: &VeryLongConfigurationRecord) -> AuthorizationFailureReason {
    let (unchanged_configuration_record, failure_reason) = split_the_configuration_record(config);
    if unchanged_configuration_record.maximum_number_of_password_attempts == 0 {
        return AuthorizationFailureReason::RedirectLocationDoesNotMatchTheRegistration;
    }
    failure_reason
}

/// A `match` ending a loop's body whose `None` arm leaves: the exit first,
/// then the rest unnested (lint refuses an `else` after `return`).
pub fn count_the_named_settings(configuration_text: &str) -> Result<u32, u32> {
    let mut number_of_settings: u32 = 0;
    for setting in configuration_text.split(';') {
        match setting.split_once('=') {
            None => return Err(number_of_settings),
            Some((setting_name, setting_value)) => {
                let name_length = setting_name.len();
                number_of_settings += 1;
                if name_length > setting_value.len() {
                    number_of_settings += 1;
                }
            }
        }
    }
    Ok(number_of_settings)
}

/// A default widened to `bigint`: `BigInt(o ?? 1)`, no cast on the `1`.
pub fn widened_attempt_limit(configured_limit: Option<u32>) -> i64 {
    i64::from(configured_limit.unwrap_or(1))
}

/// A `while` test that reads its value through a function called in place:
/// opened, one operand a line.
pub fn length_of_the_leading_digits(text_to_scan: &str) -> usize {
    let bytes_of_the_text = text_to_scan.as_bytes();
    let mut scanned_position: usize = 0;
    while scanned_position < bytes_of_the_text.len() && matches!(bytes_of_the_text[scanned_position], b'0'..=b'9') {
        scanned_position += 1;
    }
    scanned_position
}

fn the_limit_is_listed_for_the_record(listed_limits: &[u32], candidate_limit: u32, upper_bound: u32) -> bool {
    listed_limits.iter().any(|l| *l == candidate_limit) && candidate_limit < upper_bound
}

/// A `?:` branch too long for its line: broken at its loosest operator, and
/// a parenthesized `||` group inside it stays open on its line.
pub fn limits_agree_with_the_record(
    config: &VeryLongConfigurationRecord,
    listed_limits: &[u32],
    candidate_limit: u32,
    check_the_list: bool,
) -> bool {
    if check_the_list {
        (listed_limits.is_empty()
            || the_limit_is_listed_for_the_record(listed_limits, candidate_limit, config.maximum_number_of_password_attempts))
            && (config.maximum_number_of_one_time_code_attempts == 0
                || the_limit_is_listed_for_the_record(listed_limits, candidate_limit, config.minimum_length_of_the_generated_identifier))
    } else {
        config.maximum_number_of_password_attempts / 7 + config.maximum_number_of_one_time_code_attempts / 7 + 1
            == candidate_limit
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutProblem {
    Overflow,
}

/// A signature whose `=>` fits but whose `=> {` does not: the parameters
/// open (a block body never moves to the next line), not the return type.
fn encode_code_points(input: Vec<u32>) -> Result<Vec<char>, LayoutProblem> {
    if input.is_empty() {
        return Err(LayoutProblem::Overflow);
    }
    Ok(Vec::new())
}

pub fn encode_every_code_point(input: Vec<u32>) -> Result<Vec<char>, LayoutProblem> {
    encode_code_points(input)
}
