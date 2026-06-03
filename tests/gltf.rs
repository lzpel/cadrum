use cadrum::Solid;
use glam::DVec3;

/// Mesh a unit cube and serialize it to GLB bytes.
fn write_glb() -> Vec<u8> {
	let cube = Solid::cube(DVec3::ZERO, DVec3::splat(10.0));
	let mut buf = Vec::new();
	Solid::mesh(&[cube], Default::default())
		.unwrap()
		.write_gltf_binary(&mut buf)
		.expect("glb write");
	buf
}

fn u32le(bytes: &[u8], off: usize) -> usize {
	u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize
}

#[test]
fn test_glb_header_and_chunks() {
	let glb = write_glb();
	assert!(glb.len() >= 20, "GLB must hold a 12-byte header + JSON chunk header");

	// 12-byte header
	assert_eq!(&glb[0..4], b"glTF", "magic");
	assert_eq!(u32le(&glb, 4), 2, "version 2");
	assert_eq!(u32le(&glb, 8), glb.len(), "header length must equal file size");

	// JSON chunk
	let json_len = u32le(&glb, 12);
	assert_eq!(&glb[16..20], b"JSON", "chunk 0 type");
	assert_eq!(json_len % 4, 0, "JSON chunk 4-byte aligned");
	let json_end = 20 + json_len;
	let json = std::str::from_utf8(&glb[20..json_end]).expect("JSON utf-8");

	// BIN chunk
	let bin_len = u32le(&glb, json_end);
	assert_eq!(&glb[json_end + 4..json_end + 8], b"BIN\0", "chunk 1 type");
	assert_eq!(bin_len % 4, 0, "BIN chunk 4-byte aligned");
	assert!(bin_len > 0, "geometry buffer non-empty");
	assert_eq!(json_end + 8 + bin_len, glb.len(), "BIN chunk reaches EOF");

	// JSON content
	assert!(json.contains(r#""version":"2.0""#), "asset version");
	assert!(json.contains(r#""mode":4"#), "triangle primitive present");
	assert!(json.contains(r#""mode":1"#), "edge LINES primitive present");
	assert!(json.contains(r#""cadrum":"edges""#), "edge extras marker present");
}

/// With the `color` feature, faces get unlit materials.
#[cfg(feature = "color")]
#[test]
fn test_glb_has_unlit_material() {
	let glb = write_glb();
	let json_len = u32le(&glb, 12);
	let json = std::str::from_utf8(&glb[20..20 + json_len]).unwrap();
	assert!(json.contains(r#""materials""#), "materials array present");
	assert!(json.contains("KHR_materials_unlit"), "unlit extension declared");
	assert!(json.contains(r#""extensionsUsed":["KHR_materials_unlit"]"#), "extensionsUsed lists the unlit ext");
}
