#[cfg(test)]
mod tests {
    use crate::registry::{State, Registry};
    use crate::filter::Filter;
    use crate::cell_value::CellValue;

    #[test]
    fn python_registry_test_issue22() {
        let f = std::fs::read("test_data/issue22.hive").unwrap();
        let res_registry = Registry::from_bytes(&f[0..], &mut Filter { ..Default::default() });
        let registry = res_registry.unwrap();
        let reg_val = registry.hive_bin_root.unwrap().root.sub_values.into_iter().find(|val| val.value_name == "TimeZoneKeyName");
        let expected_value_content = CellValue::ValueString("W. Europe Standard Time".to_string());
        let mut reg_val = reg_val.unwrap();
        let state = State {
            file_start_pos: f.as_ptr() as usize,
            hbin_offset: 4096,
            file_buffer: &f[..]
        };
        reg_val.read_content(&state);
        assert_eq!(expected_value_content, reg_val.value_content.unwrap());
    }
}