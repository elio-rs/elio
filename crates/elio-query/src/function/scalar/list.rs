//! List functions
//!
//! - list_index: Get element at index from list
//! - list_slice: Get a slice of list

use bitvec::vec::BitVec;
use elio_common::array::*;
use elio_common::data_type::LogicalType;
use elio_common::scalar::{ListValueRef, ScalarRef, ScalarVTable};

use crate::execution::error::EvalError;
use crate::function::scalar::FunctionRegistry;
use crate::function::scalar::sig::{FuncDef, FuncImpl, FuncImplArg, FuncImplReturn};

/// list_index(list, index) -> element
/// Returns the element at the given index (0-based).
/// Negative indices count from the end (-1 is the last element).
pub fn list_index_batch(args: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    let list_arr = args[0]
        .as_list()
        .unwrap_or_else(|| panic!("expected list array got {:?} array", args[0].logical_type()));
    let idx_arr = args[1].as_variant().expect("expected variant array for index");

    // Output is element type, use AnyArrayBuilder since element type is dynamic
    let mut builder = VariantArrayBuilder::with_capacity(len, LogicalType::ANY);

    // Compute valid rows (both list and index must be non-null)
    let valid_rows = vis.clone() & list_arr.valid_map().clone() & idx_arr.valid_map().clone();

    for i in 0..len {
        if valid_rows[i] {
            let list_ref = unsafe { list_arr.get_unchecked(i) };
            let idx_ref = idx_arr.get(i).unwrap();

            // Get the index value
            let idx = match idx_ref {
                ScalarRef::Integer(idx) => idx,
                _ => {
                    return Err(EvalError::type_error(format!(
                        "list index must be integer, got {:?}",
                        idx_ref
                    )));
                }
            };

            // Handle negative indices
            let list_len = list_ref.len() as i64;
            let actual_idx = if idx < 0 { list_len + idx } else { idx };

            // Bounds check
            if actual_idx < 0 || actual_idx >= list_len {
                // Out of bounds returns null
                builder.push(None);
            } else {
                // Get element at index
                let element = list_ref.iter().nth(actual_idx as usize);
                if let Some(elem) = element {
                    builder.push(Some(elem.to_owned_scalar().as_scalar_ref()));
                } else {
                    builder.push(None);
                }
            }
        } else {
            builder.push(None);
        }
    }

    Ok(builder.finish().into())
}

// args[0]: AnyArray
pub fn any_list_index_batch(args: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    let any_arr = args[0]
        .as_variant()
        .unwrap_or_else(|| panic!("expected any array got {:?} array", args[0].logical_type()));
    let idx_arr = args[1].as_variant().expect("expected any array for index");

    // Output is element type, use AnyArrayBuilder since element type is dynamic
    let mut builder = VariantArrayBuilder::with_capacity(len, LogicalType::ANY);

    // Compute valid rows (both list and index must be non-null)
    let valid_rows = vis.clone() & any_arr.valid_map().clone() & idx_arr.valid_map().clone();

    for i in 0..len {
        if valid_rows[i] {
            let any_ref = unsafe { any_arr.get_unchecked(i) };
            let idx_ref = idx_arr.get(i).unwrap();
            let list_ref = if let Some(list_ref) = any_ref.as_list() {
                list_ref
            } else {
                return Err(EvalError::type_error(format!(
                    "expected list value got {:?}",
                    any_ref.to_string()
                )));
            };
            // Get the index value
            let idx = match idx_ref {
                ScalarRef::Integer(idx) => idx,
                _ => {
                    return Err(EvalError::type_error(format!(
                        "list index must be integer, got {:?}",
                        idx_ref
                    )));
                }
            };

            // Handle negative indices
            let list_len = list_ref.len() as i64;
            let actual_idx = if idx < 0 { list_len + idx } else { idx };

            // Bounds check
            if actual_idx < 0 || actual_idx >= list_len {
                // Out of bounds returns null
                builder.push(None);
            } else {
                // Get element at index
                let element = list_ref.iter().nth(actual_idx as usize);
                if let Some(elem) = element {
                    builder.push(Some(elem.to_owned_scalar().as_scalar_ref()));
                } else {
                    builder.push(None);
                }
            }
        } else {
            builder.push(None);
        }
    }

    Ok(builder.finish().into())
}

/// list_slice(list, start, end) -> list
/// Returns a slice of the list from start (inclusive) to end (exclusive).
/// Negative indices count from the end.
/// If start is null, defaults to 0. If end is null, defaults to list length.
pub fn list_slice_batch(args: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {
    let list_arr = args[0].as_list().expect("expected list array");
    let start_arr = args[1].as_variant().expect("expected any array for start");
    let end_arr = args[2].as_variant().expect("expected any array for end");

    // Get the inner type from the input list to create matching output builder
    let inner_logical_type = list_arr.child().logical_type();
    let output_logical_type = LogicalType::new_list(inner_logical_type.clone());
    let mut builder = output_logical_type.array_builder(len).into_list().unwrap();

    // Only list must be non-null; start/end can be null (will use defaults)
    let valid_rows = vis.clone() & list_arr.valid_map().clone();

    for i in 0..len {
        if valid_rows[i] {
            let list_ref = unsafe { list_arr.get_unchecked(i) };
            let list_len = list_ref.len() as i64;

            // Get start index (default to 0 if null)
            let start = match start_arr.get(i) {
                Some(ScalarRef::Integer(idx)) => idx,
                Some(ScalarRef::Null) | None => 0,
                Some(other) => {
                    return Err(EvalError::type_error(format!(
                        "list slice start must be integer, got {:?}",
                        other
                    )));
                }
            };

            // Get end index (default to list length if null)
            let end = match end_arr.get(i) {
                Some(ScalarRef::Integer(idx)) => idx,
                Some(ScalarRef::Null) | None => list_len,
                Some(other) => {
                    return Err(EvalError::type_error(format!(
                        "list slice end must be integer, got {:?}",
                        other
                    )));
                }
            };

            // Handle negative indices
            let actual_start = if start < 0 {
                (list_len + start).max(0)
            } else {
                start.min(list_len)
            };
            let actual_end = if end < 0 {
                (list_len + end).max(0)
            } else {
                end.min(list_len)
            };

            // Build the slice
            if actual_start >= actual_end {
                // Empty slice
                builder.push(Some(ListValueRef::Slice(&[])));
            } else {
                // Collect elements in the range
                let slice: Vec<_> = list_ref
                    .iter()
                    .skip(actual_start as usize)
                    .take((actual_end - actual_start) as usize)
                    .map(|e| e.to_owned_scalar())
                    .collect();

                let list_value = elio_common::scalar::ListValue::new(slice);
                builder.push(Some(list_value.as_scalar_ref()));
            }
        } else {
            builder.push(None);
        }
    }

    Ok(builder.finish().into())
}

pub(crate) fn register(registry: &mut FunctionRegistry) {
    // list_index(List<T>, Int|Any) -> T
    // Accept Any for index because unary operations (like -1) return Any type
    let list_index_def = FuncDef {
        name: "list_index".to_string(),
        impls: vec![
            FuncImpl::new_scalar(
                "list_index",
                vec![FuncImplArg::AnyList, FuncImplArg::Exact(LogicalType::INTEGER)],
                FuncImplReturn::ListElement(0),
                list_index_batch,
            ),
            FuncImpl::new_scalar(
                "list_index",
                vec![FuncImplArg::AnyList, FuncImplArg::Exact(LogicalType::ANY)],
                FuncImplReturn::ListElement(0),
                list_index_batch,
            ),
            FuncImpl::new_scalar(
                "list_index",
                vec![
                    FuncImplArg::Exact(LogicalType::ANY),
                    FuncImplArg::Exact(LogicalType::INTEGER),
                ],
                FuncImplReturn::ListElement(0),
                any_list_index_batch,
            ),
            FuncImpl::new_scalar(
                "list_index",
                vec![
                    FuncImplArg::Exact(LogicalType::ANY),
                    FuncImplArg::Exact(LogicalType::ANY),
                ],
                FuncImplReturn::ListElement(0),
                any_list_index_batch,
            ),
        ],
        is_agg: false,
    };
    registry.insert(list_index_def);

    // list_slice(List<T>, Int|Any, Int|Any) -> List<T>
    let list_slice_def = FuncDef {
        name: "list_slice".to_string(),
        impls: vec![
            FuncImpl::new_scalar(
                "list_slice",
                vec![
                    FuncImplArg::AnyList,
                    FuncImplArg::Exact(LogicalType::INTEGER),
                    FuncImplArg::Exact(LogicalType::INTEGER),
                ],
                FuncImplReturn::SameAsArg(0),
                list_slice_batch,
            ),
            FuncImpl::new_scalar(
                "list_slice",
                vec![
                    FuncImplArg::AnyList,
                    FuncImplArg::Exact(LogicalType::ANY),
                    FuncImplArg::Exact(LogicalType::ANY),
                ],
                FuncImplReturn::SameAsArg(0),
                list_slice_batch,
            ),
        ],
        is_agg: false,
    };
    registry.insert(list_slice_def);
}
