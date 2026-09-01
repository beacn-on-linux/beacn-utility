//! Small POD builder
//! It might, longer term, be worth pulling in pipewire-natives SPA stuff, just to simplify
//! this 'ported from pipewire' crap :D

#![allow(non_upper_case_globals)]

pub const SPA_TYPE_Object: u32 = 15;
pub const SPA_TYPE_Id: u32 = 3;
pub const SPA_TYPE_Int: u32 = 4;
pub const SPA_TYPE_Array: u32 = 13;
pub const SPA_TYPE_OBJECT_Format: u32 = 0x40003;

pub const SPA_PARAM_EnumFormat: u32 = 3;

pub const SPA_FORMAT_mediaType: u32 = 1;
pub const SPA_FORMAT_mediaSubtype: u32 = 2;
pub const SPA_FORMAT_AUDIO_format: u32 = 0x10001;
pub const SPA_FORMAT_AUDIO_rate: u32 = 0x10003;
pub const SPA_FORMAT_AUDIO_channels: u32 = 0x10004;
pub const SPA_FORMAT_AUDIO_position: u32 = 0x10005;

pub const SPA_MEDIA_TYPE_audio: u32 = 1;
pub const SPA_MEDIA_SUBTYPE_raw: u32 = 1;

// Interleaved, we don't actually want this..
#[allow(unused)]
pub const SPA_AUDIO_FORMAT_F32le: u32 = 0x11b;

// We want planar so the samples are split in the buffer.
pub const SPA_AUDIO_FORMAT_F32P: u32 = 0x206;

// ---------------------------------------------------------------------

fn pad8(n: usize) -> usize {
    (n + 7) & !7
}

// Small wrapper over the buffer, used for sending stuff
struct PodBuilder {
    buf: Vec<u8>,
}

// Now this is POD Building?
impl PodBuilder {
    fn new() -> Self {
        Self {
            buf: Vec::with_capacity(128),
        }
    }

    fn write_u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_ne_bytes());
    }
    fn write_i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_ne_bytes());
    }
    fn pad_to(&mut self, boundary: usize) {
        let target = pad8(self.buf.len().max(boundary));
        while self.buf.len() < target {
            self.buf.push(0);
        }
    }

    /// Appends one `key: Id(value)` property (4-byte id payload).
    fn prop_id(&mut self, key: u32, value: u32) {
        self.write_u32(key); // spa_pod_prop.key
        self.write_u32(0); // spa_pod_prop.flags
        self.write_u32(4); // value.pod.size (payload bytes only)
        self.write_u32(SPA_TYPE_Id); // value.pod.type
        self.write_u32(value); // payload
        let mark = self.buf.len();
        self.pad_to(mark); // pad value payload to 8-byte alignment
    }

    /// Appends one `key: Int(value)` property.
    fn prop_int(&mut self, key: u32, value: i32) {
        self.write_u32(key);
        self.write_u32(0);
        self.write_u32(4);
        self.write_u32(SPA_TYPE_Int);
        self.write_i32(value);
        let mark = self.buf.len();
        self.pad_to(mark);
    }

    fn prop_id_array(&mut self, key: u32, values: &[u32]) {
        self.write_u32(key); // prop.key
        self.write_u32(0); // prop.flags

        let array_body_size = 8 + values.len() * 4;

        self.write_u32(array_body_size as u32);
        self.write_u32(SPA_TYPE_Array);

        // Array child descriptor.
        self.write_u32(4); // child.size
        self.write_u32(SPA_TYPE_Id); // child.type

        // Array elements.
        for &value in values {
            self.write_u32(value);
        }

        // PODs are aligned to 8 bytes.
        let mark = self.buf.len();
        self.pad_to(mark);
    }
}

// This builds out with some static values and gets us running. We accept channels
// as their 'expected' name (such as FL, FR, AUX0 etc), just so we manage the remapping
// internally.
pub fn build_audio_pod<S: AsRef<str>>(rate: u32, channels: &[S]) -> Vec<u8> {
    let channel_ids: Vec<u32> = channels
        .iter()
        .map(|name| {
            channel_id(name.as_ref())
                .unwrap_or_else(|| panic!("unknown SPA audio channel: {}", name.as_ref()))
        })
        .collect();

    let mut object_body = PodBuilder::new();
    object_body.write_u32(SPA_TYPE_OBJECT_Format);
    object_body.write_u32(SPA_PARAM_EnumFormat);
    object_body.prop_id(SPA_FORMAT_mediaType, SPA_MEDIA_TYPE_audio);
    object_body.prop_id(SPA_FORMAT_mediaSubtype, SPA_MEDIA_SUBTYPE_raw);
    object_body.prop_id(SPA_FORMAT_AUDIO_format, SPA_AUDIO_FORMAT_F32P);
    object_body.prop_int(SPA_FORMAT_AUDIO_rate, rate as i32);
    object_body.prop_int(SPA_FORMAT_AUDIO_channels, channel_ids.len() as i32);
    object_body.prop_id_array(SPA_FORMAT_AUDIO_position, &channel_ids);

    let mut out = Vec::with_capacity(8 + object_body.buf.len());
    out.extend_from_slice(&(object_body.buf.len() as u32).to_ne_bytes());
    out.extend_from_slice(&SPA_TYPE_Object.to_ne_bytes());
    out.extend_from_slice(&object_body.buf);

    out
}

// Fuck everything about this :D
fn channel_id(name: &str) -> Option<u32> {
    match name {
        "UNKNOWN" => Some(SPA_AUDIO_CHANNEL_UNKNOWN),
        "NA" => Some(SPA_AUDIO_CHANNEL_NA),
        "MONO" => Some(SPA_AUDIO_CHANNEL_MONO),
        "FL" => Some(SPA_AUDIO_CHANNEL_FL),
        "FR" => Some(SPA_AUDIO_CHANNEL_FR),
        "FC" => Some(SPA_AUDIO_CHANNEL_FC),
        "LFE" => Some(SPA_AUDIO_CHANNEL_LFE),
        "SL" => Some(SPA_AUDIO_CHANNEL_SL),
        "SR" => Some(SPA_AUDIO_CHANNEL_SR),
        "FLC" => Some(SPA_AUDIO_CHANNEL_FLC),
        "FRC" => Some(SPA_AUDIO_CHANNEL_FRC),
        "RC" => Some(SPA_AUDIO_CHANNEL_RC),
        "RL" => Some(SPA_AUDIO_CHANNEL_RL),
        "RR" => Some(SPA_AUDIO_CHANNEL_RR),
        "TC" => Some(SPA_AUDIO_CHANNEL_TC),
        "TFL" => Some(SPA_AUDIO_CHANNEL_TFL),
        "TFC" => Some(SPA_AUDIO_CHANNEL_TFC),
        "TFR" => Some(SPA_AUDIO_CHANNEL_TFR),
        "TRL" => Some(SPA_AUDIO_CHANNEL_TRL),
        "TRC" => Some(SPA_AUDIO_CHANNEL_TRC),
        "TRR" => Some(SPA_AUDIO_CHANNEL_TRR),
        "RLC" => Some(SPA_AUDIO_CHANNEL_RLC),
        "RRC" => Some(SPA_AUDIO_CHANNEL_RRC),
        "FLW" => Some(SPA_AUDIO_CHANNEL_FLW),
        "FRW" => Some(SPA_AUDIO_CHANNEL_FRW),
        "LFE2" => Some(SPA_AUDIO_CHANNEL_LFE2),
        "FLH" => Some(SPA_AUDIO_CHANNEL_FLH),
        "FCH" => Some(SPA_AUDIO_CHANNEL_FCH),
        "FRH" => Some(SPA_AUDIO_CHANNEL_FRH),
        "TFLC" => Some(SPA_AUDIO_CHANNEL_TFLC),
        "TFRC" => Some(SPA_AUDIO_CHANNEL_TFRC),
        "TSL" => Some(SPA_AUDIO_CHANNEL_TSL),
        "TSR" => Some(SPA_AUDIO_CHANNEL_TSR),
        "LLFE" => Some(SPA_AUDIO_CHANNEL_LLFE),
        "RLFE" => Some(SPA_AUDIO_CHANNEL_RLFE),
        "BC" => Some(SPA_AUDIO_CHANNEL_BC),
        "BLC" => Some(SPA_AUDIO_CHANNEL_BLC),
        "BRC" => Some(SPA_AUDIO_CHANNEL_BRC),

        "AUX0" => Some(SPA_AUDIO_CHANNEL_AUX0),
        "AUX1" => Some(SPA_AUDIO_CHANNEL_AUX1),
        "AUX2" => Some(SPA_AUDIO_CHANNEL_AUX2),
        "AUX3" => Some(SPA_AUDIO_CHANNEL_AUX3),
        "AUX4" => Some(SPA_AUDIO_CHANNEL_AUX4),
        "AUX5" => Some(SPA_AUDIO_CHANNEL_AUX5),
        "AUX6" => Some(SPA_AUDIO_CHANNEL_AUX6),
        "AUX7" => Some(SPA_AUDIO_CHANNEL_AUX7),
        "AUX8" => Some(SPA_AUDIO_CHANNEL_AUX8),
        "AUX9" => Some(SPA_AUDIO_CHANNEL_AUX9),
        "AUX10" => Some(SPA_AUDIO_CHANNEL_AUX10),
        "AUX11" => Some(SPA_AUDIO_CHANNEL_AUX11),
        "AUX12" => Some(SPA_AUDIO_CHANNEL_AUX12),
        "AUX13" => Some(SPA_AUDIO_CHANNEL_AUX13),
        "AUX14" => Some(SPA_AUDIO_CHANNEL_AUX14),
        "AUX15" => Some(SPA_AUDIO_CHANNEL_AUX15),
        "AUX16" => Some(SPA_AUDIO_CHANNEL_AUX16),
        "AUX17" => Some(SPA_AUDIO_CHANNEL_AUX17),
        "AUX18" => Some(SPA_AUDIO_CHANNEL_AUX18),
        "AUX19" => Some(SPA_AUDIO_CHANNEL_AUX19),
        "AUX20" => Some(SPA_AUDIO_CHANNEL_AUX20),
        "AUX21" => Some(SPA_AUDIO_CHANNEL_AUX21),
        "AUX22" => Some(SPA_AUDIO_CHANNEL_AUX22),
        "AUX23" => Some(SPA_AUDIO_CHANNEL_AUX23),
        "AUX24" => Some(SPA_AUDIO_CHANNEL_AUX24),
        "AUX25" => Some(SPA_AUDIO_CHANNEL_AUX25),
        "AUX26" => Some(SPA_AUDIO_CHANNEL_AUX26),
        "AUX27" => Some(SPA_AUDIO_CHANNEL_AUX27),
        "AUX28" => Some(SPA_AUDIO_CHANNEL_AUX28),
        "AUX29" => Some(SPA_AUDIO_CHANNEL_AUX29),
        "AUX30" => Some(SPA_AUDIO_CHANNEL_AUX30),
        "AUX31" => Some(SPA_AUDIO_CHANNEL_AUX31),
        "AUX32" => Some(SPA_AUDIO_CHANNEL_AUX32),
        "AUX33" => Some(SPA_AUDIO_CHANNEL_AUX33),
        "AUX34" => Some(SPA_AUDIO_CHANNEL_AUX34),
        "AUX35" => Some(SPA_AUDIO_CHANNEL_AUX35),
        "AUX36" => Some(SPA_AUDIO_CHANNEL_AUX36),
        "AUX37" => Some(SPA_AUDIO_CHANNEL_AUX37),
        "AUX38" => Some(SPA_AUDIO_CHANNEL_AUX38),
        "AUX39" => Some(SPA_AUDIO_CHANNEL_AUX39),
        "AUX40" => Some(SPA_AUDIO_CHANNEL_AUX40),
        "AUX41" => Some(SPA_AUDIO_CHANNEL_AUX41),
        "AUX42" => Some(SPA_AUDIO_CHANNEL_AUX42),
        "AUX43" => Some(SPA_AUDIO_CHANNEL_AUX43),
        "AUX44" => Some(SPA_AUDIO_CHANNEL_AUX44),
        "AUX45" => Some(SPA_AUDIO_CHANNEL_AUX45),
        "AUX46" => Some(SPA_AUDIO_CHANNEL_AUX46),
        "AUX47" => Some(SPA_AUDIO_CHANNEL_AUX47),
        "AUX48" => Some(SPA_AUDIO_CHANNEL_AUX48),
        "AUX49" => Some(SPA_AUDIO_CHANNEL_AUX49),
        "AUX50" => Some(SPA_AUDIO_CHANNEL_AUX50),
        "AUX51" => Some(SPA_AUDIO_CHANNEL_AUX51),
        "AUX52" => Some(SPA_AUDIO_CHANNEL_AUX52),
        "AUX53" => Some(SPA_AUDIO_CHANNEL_AUX53),
        "AUX54" => Some(SPA_AUDIO_CHANNEL_AUX54),
        "AUX55" => Some(SPA_AUDIO_CHANNEL_AUX55),
        "AUX56" => Some(SPA_AUDIO_CHANNEL_AUX56),
        "AUX57" => Some(SPA_AUDIO_CHANNEL_AUX57),
        "AUX58" => Some(SPA_AUDIO_CHANNEL_AUX58),
        "AUX59" => Some(SPA_AUDIO_CHANNEL_AUX59),
        "AUX60" => Some(SPA_AUDIO_CHANNEL_AUX60),
        "AUX61" => Some(SPA_AUDIO_CHANNEL_AUX61),
        "AUX62" => Some(SPA_AUDIO_CHANNEL_AUX62),
        "AUX63" => Some(SPA_AUDIO_CHANNEL_AUX63),

        _ => None,
    }
}

pub const SPA_AUDIO_CHANNEL_UNKNOWN: u32 = 0;
pub const SPA_AUDIO_CHANNEL_NA: u32 = 1;
pub const SPA_AUDIO_CHANNEL_MONO: u32 = 2;
pub const SPA_AUDIO_CHANNEL_FL: u32 = 3;
pub const SPA_AUDIO_CHANNEL_FR: u32 = 4;
pub const SPA_AUDIO_CHANNEL_FC: u32 = 5;
pub const SPA_AUDIO_CHANNEL_LFE: u32 = 6;
pub const SPA_AUDIO_CHANNEL_SL: u32 = 7;
pub const SPA_AUDIO_CHANNEL_SR: u32 = 8;
pub const SPA_AUDIO_CHANNEL_FLC: u32 = 9;
pub const SPA_AUDIO_CHANNEL_FRC: u32 = 10;
pub const SPA_AUDIO_CHANNEL_RC: u32 = 11;
pub const SPA_AUDIO_CHANNEL_RL: u32 = 12;
pub const SPA_AUDIO_CHANNEL_RR: u32 = 13;
pub const SPA_AUDIO_CHANNEL_TC: u32 = 14;
pub const SPA_AUDIO_CHANNEL_TFL: u32 = 15;
pub const SPA_AUDIO_CHANNEL_TFC: u32 = 16;
pub const SPA_AUDIO_CHANNEL_TFR: u32 = 17;
pub const SPA_AUDIO_CHANNEL_TRL: u32 = 18;
pub const SPA_AUDIO_CHANNEL_TRC: u32 = 19;
pub const SPA_AUDIO_CHANNEL_TRR: u32 = 20;
pub const SPA_AUDIO_CHANNEL_RLC: u32 = 21;
pub const SPA_AUDIO_CHANNEL_RRC: u32 = 22;
pub const SPA_AUDIO_CHANNEL_FLW: u32 = 23;
pub const SPA_AUDIO_CHANNEL_FRW: u32 = 24;
pub const SPA_AUDIO_CHANNEL_LFE2: u32 = 25;
pub const SPA_AUDIO_CHANNEL_FLH: u32 = 26;
pub const SPA_AUDIO_CHANNEL_FCH: u32 = 27;
pub const SPA_AUDIO_CHANNEL_FRH: u32 = 28;
pub const SPA_AUDIO_CHANNEL_TFLC: u32 = 29;
pub const SPA_AUDIO_CHANNEL_TFRC: u32 = 30;
pub const SPA_AUDIO_CHANNEL_TSL: u32 = 31;
pub const SPA_AUDIO_CHANNEL_TSR: u32 = 32;
pub const SPA_AUDIO_CHANNEL_LLFE: u32 = 33;
pub const SPA_AUDIO_CHANNEL_RLFE: u32 = 34;
pub const SPA_AUDIO_CHANNEL_BC: u32 = 35;
pub const SPA_AUDIO_CHANNEL_BLC: u32 = 36;
pub const SPA_AUDIO_CHANNEL_BRC: u32 = 37;
pub const SPA_AUDIO_CHANNEL_AUX0: u32 = 4096;
pub const SPA_AUDIO_CHANNEL_AUX1: u32 = 4097;
pub const SPA_AUDIO_CHANNEL_AUX2: u32 = 4098;
pub const SPA_AUDIO_CHANNEL_AUX3: u32 = 4099;
pub const SPA_AUDIO_CHANNEL_AUX4: u32 = 4100;
pub const SPA_AUDIO_CHANNEL_AUX5: u32 = 4101;
pub const SPA_AUDIO_CHANNEL_AUX6: u32 = 4102;
pub const SPA_AUDIO_CHANNEL_AUX7: u32 = 4103;
pub const SPA_AUDIO_CHANNEL_AUX8: u32 = 4104;
pub const SPA_AUDIO_CHANNEL_AUX9: u32 = 4105;
pub const SPA_AUDIO_CHANNEL_AUX10: u32 = 4106;
pub const SPA_AUDIO_CHANNEL_AUX11: u32 = 4107;
pub const SPA_AUDIO_CHANNEL_AUX12: u32 = 4108;
pub const SPA_AUDIO_CHANNEL_AUX13: u32 = 4109;
pub const SPA_AUDIO_CHANNEL_AUX14: u32 = 4110;
pub const SPA_AUDIO_CHANNEL_AUX15: u32 = 4111;
pub const SPA_AUDIO_CHANNEL_AUX16: u32 = 4112;
pub const SPA_AUDIO_CHANNEL_AUX17: u32 = 4113;
pub const SPA_AUDIO_CHANNEL_AUX18: u32 = 4114;
pub const SPA_AUDIO_CHANNEL_AUX19: u32 = 4115;
pub const SPA_AUDIO_CHANNEL_AUX20: u32 = 4116;
pub const SPA_AUDIO_CHANNEL_AUX21: u32 = 4117;
pub const SPA_AUDIO_CHANNEL_AUX22: u32 = 4118;
pub const SPA_AUDIO_CHANNEL_AUX23: u32 = 4119;
pub const SPA_AUDIO_CHANNEL_AUX24: u32 = 4120;
pub const SPA_AUDIO_CHANNEL_AUX25: u32 = 4121;
pub const SPA_AUDIO_CHANNEL_AUX26: u32 = 4122;
pub const SPA_AUDIO_CHANNEL_AUX27: u32 = 4123;
pub const SPA_AUDIO_CHANNEL_AUX28: u32 = 4124;
pub const SPA_AUDIO_CHANNEL_AUX29: u32 = 4125;
pub const SPA_AUDIO_CHANNEL_AUX30: u32 = 4126;
pub const SPA_AUDIO_CHANNEL_AUX31: u32 = 4127;
pub const SPA_AUDIO_CHANNEL_AUX32: u32 = 4128;
pub const SPA_AUDIO_CHANNEL_AUX33: u32 = 4129;
pub const SPA_AUDIO_CHANNEL_AUX34: u32 = 4130;
pub const SPA_AUDIO_CHANNEL_AUX35: u32 = 4131;
pub const SPA_AUDIO_CHANNEL_AUX36: u32 = 4132;
pub const SPA_AUDIO_CHANNEL_AUX37: u32 = 4133;
pub const SPA_AUDIO_CHANNEL_AUX38: u32 = 4134;
pub const SPA_AUDIO_CHANNEL_AUX39: u32 = 4135;
pub const SPA_AUDIO_CHANNEL_AUX40: u32 = 4136;
pub const SPA_AUDIO_CHANNEL_AUX41: u32 = 4137;
pub const SPA_AUDIO_CHANNEL_AUX42: u32 = 4138;
pub const SPA_AUDIO_CHANNEL_AUX43: u32 = 4139;
pub const SPA_AUDIO_CHANNEL_AUX44: u32 = 4140;
pub const SPA_AUDIO_CHANNEL_AUX45: u32 = 4141;
pub const SPA_AUDIO_CHANNEL_AUX46: u32 = 4142;
pub const SPA_AUDIO_CHANNEL_AUX47: u32 = 4143;
pub const SPA_AUDIO_CHANNEL_AUX48: u32 = 4144;
pub const SPA_AUDIO_CHANNEL_AUX49: u32 = 4145;
pub const SPA_AUDIO_CHANNEL_AUX50: u32 = 4146;
pub const SPA_AUDIO_CHANNEL_AUX51: u32 = 4147;
pub const SPA_AUDIO_CHANNEL_AUX52: u32 = 4148;
pub const SPA_AUDIO_CHANNEL_AUX53: u32 = 4149;
pub const SPA_AUDIO_CHANNEL_AUX54: u32 = 4150;
pub const SPA_AUDIO_CHANNEL_AUX55: u32 = 4151;
pub const SPA_AUDIO_CHANNEL_AUX56: u32 = 4152;
pub const SPA_AUDIO_CHANNEL_AUX57: u32 = 4153;
pub const SPA_AUDIO_CHANNEL_AUX58: u32 = 4154;
pub const SPA_AUDIO_CHANNEL_AUX59: u32 = 4155;
pub const SPA_AUDIO_CHANNEL_AUX60: u32 = 4156;
pub const SPA_AUDIO_CHANNEL_AUX61: u32 = 4157;
pub const SPA_AUDIO_CHANNEL_AUX62: u32 = 4158;
pub const SPA_AUDIO_CHANNEL_AUX63: u32 = 4159;
