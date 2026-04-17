pub fn build_bc_protocol_frame(value: u16) -> [u8; 10] {
    let [hi, lo] = value.to_be_bytes();
    [0xBC, 0x04, 0x06, hi, lo, 0x03, 0xE8, 0x00, 0x00, 0x55]
}
