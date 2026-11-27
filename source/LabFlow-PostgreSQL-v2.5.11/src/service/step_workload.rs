//! Quantity rules for a single detection. Callers must serialize changes in a transaction.

#[derive(Debug, Clone, Copy)]
pub struct StepEntry {
    pub group: usize,
    pub step: usize,
    pub quantity: i32,
    pub rework: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Progress {
    pub normal_quantities: Vec<Vec<i32>>,
    pub complete: bool,
}

/// Validates the entire resulting state, including edits and restores.
pub fn progress(
    total: i32,
    groups: &[i32],
    steps: usize,
    entries: &[StepEntry],
    ordered: bool,
) -> Result<Progress, &'static str> {
    if total <= 0 || groups.is_empty() || steps == 0 {
        return Err("检测数量、分组和步骤必须有效");
    }
    let mut grouped = 0_i32;
    for &quantity in groups {
        if quantity <= 0 {
            return Err("分组数量必须大于0");
        }
        grouped = grouped.checked_add(quantity).ok_or("分组数量超出范围")?;
    }
    if grouped > total {
        return Err("分组数量不能超过检测数量");
    }
    let mut quantities = vec![vec![0_i32; steps]; groups.len()];
    for entry in entries {
        if entry.group >= groups.len() || entry.step >= steps || entry.quantity <= 0 {
            return Err("步骤、分组或处理数量无效");
        }
        if !entry.rework {
            let quantity = &mut quantities[entry.group][entry.step];
            *quantity = quantity
                .checked_add(entry.quantity)
                .ok_or("步骤数量超出范围")?;
            if *quantity > groups[entry.group] {
                return Err("步骤分工数量不能超过分组数量");
            }
        }
    }
    if ordered
        && quantities
            .iter()
            .any(|row| row.windows(2).any(|pair| pair[1] > pair[0]))
    {
        return Err("后续步骤数量不能超过前一步骤数量");
    }
    let complete = grouped == total
        && quantities
            .iter()
            .zip(groups)
            .all(|(row, &group)| row.iter().all(|&quantity| quantity == group));
    Ok(Progress {
        normal_quantities: quantities,
        complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(group: usize, step: usize, quantity: i32) -> StepEntry {
        StepEntry {
            group,
            step,
            quantity,
            rework: false,
        }
    }

    #[test]
    fn whole_detection_completes_only_after_all_groups_and_steps() {
        let mut entries = vec![entry(0, 0, 5), entry(0, 1, 5), entry(0, 2, 5)];
        assert!(!progress(10, &[5, 5], 3, &entries, true).unwrap().complete);
        entries.extend([
            entry(1, 0, 2),
            entry(1, 0, 3),
            entry(1, 1, 5),
            entry(1, 2, 5),
        ]);
        assert!(progress(10, &[5, 5], 3, &entries, true).unwrap().complete);
        assert!(!progress(10, &[5], 3, &entries[..3], true).unwrap().complete);
    }

    #[test]
    fn capacity_order_and_rework_are_independent() {
        assert!(progress(10, &[10], 2, &[entry(0, 0, 6), entry(0, 0, 5)], false).is_err());
        assert!(progress(10, &[10], 2, &[entry(0, 0, 5), entry(0, 1, 6)], true).is_err());
        assert!(progress(10, &[10], 2, &[entry(0, 0, 5), entry(0, 1, 6)], false).is_ok());
        let rework = StepEntry {
            rework: true,
            ..entry(0, 0, 20)
        };
        assert!(!progress(10, &[10], 2, &[rework], false).unwrap().complete);
        assert!(
            progress(
                10,
                &[10],
                2,
                &[entry(0, 0, 10), entry(0, 1, 10), rework],
                true
            )
            .unwrap()
            .complete
        );
    }

    #[test]
    fn three_steps_do_not_triple_the_sample_count() {
        let result = progress(
            10,
            &[10],
            3,
            &[entry(0, 0, 10), entry(0, 1, 10), entry(0, 2, 10)],
            true,
        )
        .unwrap();
        assert!(result.complete);
        assert_eq!(result.normal_quantities, vec![vec![10, 10, 10]]);
    }

    #[test]
    fn removing_or_restoring_a_step_changes_completion() {
        let mut entries = vec![entry(0, 0, 10), entry(0, 1, 10), entry(0, 2, 10)];
        assert!(progress(10, &[10], 3, &entries, true).unwrap().complete);
        let removed = entries.pop().unwrap();
        assert!(!progress(10, &[10], 3, &entries, true).unwrap().complete);
        entries.push(removed);
        assert!(progress(10, &[10], 3, &entries, true).unwrap().complete);
    }

    #[test]
    fn rejects_invalid_and_overflowing_inputs() {
        assert!(progress(10, &[6, 5], 1, &[], false).is_err());
        assert!(progress(10, &[10], 0, &[], false).is_err());
        assert!(progress(10, &[10], 1, &[entry(1, 0, 1)], false).is_err());
        assert!(progress(i32::MAX, &[i32::MAX, 1], 1, &[], false).is_err());
        assert!(progress(
            i32::MAX,
            &[i32::MAX],
            1,
            &[entry(0, 0, i32::MAX), entry(0, 0, 1)],
            false
        )
        .is_err());
    }
}
