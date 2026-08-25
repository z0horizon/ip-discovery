//! STUN message encoding/decoding
//!
//! Implements minimal RFC 5389 STUN message format for binding requests.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// STUN magic cookie (RFC 5389)
const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN message types (RFC 5389)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunMethod {
    /// Binding request
    Request,
    /// Binding success response
    Response,
    /// Binding error response
    Error,
}

impl StunMethod {
    fn to_u16(self) -> u16 {
        match self {
            StunMethod::Request => 0x0001,
            StunMethod::Response => 0x0101,
            StunMethod::Error => 0x0111,
        }
    }

    fn from_u16(value: u16) -> Option<Self> {
        match value {
            0x0001 => Some(StunMethod::Request),
            0x0101 => Some(StunMethod::Response),
            0x0111 => Some(StunMethod::Error),
            _ => None,
        }
    }
}

/// STUN attribute types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttributeType {
    MappedAddress,
    XorMappedAddress,
    Unknown(u16),
}

impl AttributeType {
    fn from_u16(value: u16) -> Self {
        match value {
            0x0001 => AttributeType::MappedAddress,
            0x0020 => AttributeType::XorMappedAddress,
            other => AttributeType::Unknown(other),
        }
    }
}

/// A STUN protocol message (RFC 5389).
///
/// Supports encoding binding requests and decoding binding responses
/// to extract the mapped (public) IP address.
#[derive(Debug)]
pub struct StunMessage {
    msg_type: StunMethod,
    transaction_id: [u8; 12],
    attributes: Vec<(AttributeType, Vec<u8>)>,
}

impl StunMessage {
    /// Create a new STUN message
    pub fn new(msg_type: StunMethod) -> Result<Self, getrandom::Error> {
        let mut transaction_id = [0u8; 12];
        getrandom::fill(&mut transaction_id)?;

        Ok(Self {
            msg_type,
            transaction_id,
            attributes: Vec::new(),
        })
    }

    /// Return the STUN message class encoded in the header.
    pub fn method(&self) -> StunMethod {
        self.msg_type
    }

    /// Get transaction ID
    pub fn transaction_id(&self) -> &[u8; 12] {
        &self.transaction_id
    }

    /// Encode this message to its wire format (20 bytes for a binding request).
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(20);

        // Message type (2 bytes)
        buf.extend_from_slice(&self.msg_type.to_u16().to_be_bytes());

        // Message length (2 bytes) - 0 for binding request with no attributes
        buf.extend_from_slice(&0u16.to_be_bytes());

        // Magic cookie (4 bytes)
        buf.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());

        // Transaction ID (12 bytes)
        buf.extend_from_slice(&self.transaction_id);

        buf
    }

    /// Decode a STUN message from its wire format.
    ///
    /// Returns an error if the data is too short, has an invalid magic cookie,
    /// or contains an unrecognized message type.
    pub fn decode(data: &[u8]) -> Result<Self, &'static str> {
        if data.len() < 20 {
            return Err("message too short");
        }

        // Parse header
        let msg_type_raw = u16::from_be_bytes([data[0], data[1]]);
        let msg_type = StunMethod::from_u16(msg_type_raw).ok_or("unknown message type")?;

        let msg_length = u16::from_be_bytes([data[2], data[3]]) as usize;

        let magic = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if magic != MAGIC_COOKIE {
            return Err("invalid magic cookie");
        }

        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);

        // Parse attributes
        if msg_length % 4 != 0 {
            return Err("invalid message length");
        }

        let end = 20usize
            .checked_add(msg_length)
            .ok_or("message length overflow")?;
        if data.len() < end {
            return Err("truncated message");
        }

        let mut attributes = Vec::new();
        let mut offset = 20;

        while offset < end {
            if offset.checked_add(4).ok_or("attribute length overflow")? > end {
                return Err("truncated attribute header");
            }
            let attr_type_raw = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let attr_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;

            offset += 4;
            let value_end = offset
                .checked_add(attr_len)
                .ok_or("attribute length overflow")?;
            if value_end > end {
                return Err("attribute exceeds message length");
            }

            let attr_type = AttributeType::from_u16(attr_type_raw);
            let attr_value = data[offset..value_end].to_vec();
            attributes.push((attr_type, attr_value));

            // Padding to 4-byte boundary
            let padding = (4 - (attr_len % 4)) % 4;
            offset = value_end
                .checked_add(padding)
                .ok_or("attribute length overflow")?;
            if offset > end {
                return Err("invalid attribute padding");
            }
        }

        Ok(Self {
            msg_type,
            transaction_id,
            attributes,
        })
    }

    /// Extract the mapped (public) IP address from the response.
    ///
    /// Prefers XOR-MAPPED-ADDRESS over MAPPED-ADDRESS per RFC 5389 recommendation.
    pub fn get_mapped_address(&self) -> Option<IpAddr> {
        // Try XOR-MAPPED-ADDRESS first (preferred)
        for (attr_type, value) in &self.attributes {
            if *attr_type == AttributeType::XorMappedAddress {
                return self.parse_xor_mapped_address(value);
            }
        }

        // Fall back to MAPPED-ADDRESS
        for (attr_type, value) in &self.attributes {
            if *attr_type == AttributeType::MappedAddress {
                return self.parse_mapped_address(value);
            }
        }

        None
    }

    fn parse_mapped_address(&self, data: &[u8]) -> Option<IpAddr> {
        if data.len() < 4 {
            return None;
        }

        let family = data[1];

        match family {
            0x01 => {
                // IPv4
                if data.len() < 8 {
                    return None;
                }
                let addr = Ipv4Addr::new(data[4], data[5], data[6], data[7]);
                Some(IpAddr::V4(addr))
            }
            0x02 => {
                // IPv6
                if data.len() < 20 {
                    return None;
                }
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&data[4..20]);
                Some(IpAddr::V6(Ipv6Addr::from(octets)))
            }
            _ => None,
        }
    }

    fn parse_xor_mapped_address(&self, data: &[u8]) -> Option<IpAddr> {
        if data.len() < 4 {
            return None;
        }

        let family = data[1];

        match family {
            0x01 => {
                // IPv4 - XOR with magic cookie
                if data.len() < 8 {
                    return None;
                }
                let xor_addr = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
                let addr = xor_addr ^ MAGIC_COOKIE;
                Some(IpAddr::V4(Ipv4Addr::from(addr)))
            }
            0x02 => {
                // IPv6 - XOR with magic cookie + transaction ID
                if data.len() < 20 {
                    return None;
                }

                let mut xor_key = [0u8; 16];
                xor_key[0..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
                xor_key[4..16].copy_from_slice(&self.transaction_id);

                let mut octets = [0u8; 16];
                for i in 0..16 {
                    octets[i] = data[4 + i] ^ xor_key[i];
                }

                Some(IpAddr::V6(Ipv6Addr::from(octets)))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Encoding ────────────────────────────────────────────────────

    #[test]
    fn test_encode_binding_request() {
        let msg = StunMessage::new(StunMethod::Request).unwrap();
        let encoded = msg.encode();

        assert_eq!(encoded.len(), 20);
        // Message type: Binding Request (0x0001)
        assert_eq!(encoded[0], 0x00);
        assert_eq!(encoded[1], 0x01);
        // Message length: 0 (no attributes)
        assert_eq!(encoded[2], 0x00);
        assert_eq!(encoded[3], 0x00);
        // Magic Cookie
        assert_eq!(&encoded[4..8], &[0x21, 0x12, 0xA4, 0x42]);
    }

    #[test]
    fn test_encode_preserves_transaction_id() {
        let msg = StunMessage::new(StunMethod::Request).unwrap();
        let encoded = msg.encode();
        assert_eq!(&encoded[8..20], msg.transaction_id());
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = StunMessage::new(StunMethod::Request).unwrap();
        let encoded = original.encode();
        let decoded = StunMessage::decode(&encoded).unwrap();

        assert_eq!(decoded.msg_type, StunMethod::Request);
        assert_eq!(decoded.transaction_id(), original.transaction_id());
        assert!(decoded.attributes.is_empty());
    }

    // ── StunMethod ──────────────────────────────────────────────────

    #[test]
    fn test_stun_method_round_trip() {
        assert_eq!(StunMethod::from_u16(0x0001), Some(StunMethod::Request));
        assert_eq!(StunMethod::from_u16(0x0101), Some(StunMethod::Response));
        assert_eq!(StunMethod::from_u16(0x0111), Some(StunMethod::Error));
        assert_eq!(StunMethod::from_u16(0xFFFF), None);
    }

    #[test]
    fn test_stun_method_to_u16() {
        assert_eq!(StunMethod::Request.to_u16(), 0x0001);
        assert_eq!(StunMethod::Response.to_u16(), 0x0101);
        assert_eq!(StunMethod::Error.to_u16(), 0x0111);
    }

    // ── Decode error paths ──────────────────────────────────────────

    #[test]
    fn test_decode_too_short() {
        let data = [0u8; 19];
        assert!(matches!(
            StunMessage::decode(&data),
            Err("message too short")
        ));
    }

    #[test]
    fn test_decode_bad_magic_cookie() {
        let mut data = [0u8; 20];
        data[0] = 0x00;
        data[1] = 0x01; // Binding Request
                        // Leave magic cookie as 0x00000000 (wrong)
        assert!(matches!(
            StunMessage::decode(&data),
            Err("invalid magic cookie")
        ));
    }

    #[test]
    fn test_decode_unknown_message_type() {
        let mut data = [0u8; 20];
        data[0] = 0xFF;
        data[1] = 0xFF; // Unknown type
        data[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        assert!(matches!(
            StunMessage::decode(&data),
            Err("unknown message type")
        ));
    }

    #[test]
    fn test_decode_rejects_truncated_declared_message() {
        let mut data = Vec::from([
            0x01, 0x01, 0x00, 0x0c, 0x21, 0x12, 0xa4, 0x42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        data.extend_from_slice(&[0x00, 0x20, 0x00, 0x08]);

        assert!(matches!(
            StunMessage::decode(&data),
            Err("truncated message")
        ));
    }

    #[test]
    fn test_decode_rejects_attribute_beyond_declared_message() {
        let mut data = Vec::from([
            0x01, 0x01, 0x00, 0x04, 0x21, 0x12, 0xa4, 0x42, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        data.extend_from_slice(&[0x00, 0x20, 0x00, 0x08]);

        assert!(matches!(
            StunMessage::decode(&data),
            Err("attribute exceeds message length")
        ));
    }

    // ── XOR-MAPPED-ADDRESS (IPv4) ───────────────────────────────────

    #[test]
    fn test_xor_mapped_address_ipv4() {
        let tx_id = [0x00u8; 12];
        let ip = Ipv4Addr::new(203, 0, 113, 42);
        let xor_addr = u32::from(ip) ^ MAGIC_COOKIE;

        let mut response = Vec::with_capacity(32);
        response.extend_from_slice(&0x0101u16.to_be_bytes()); // Binding Response
        response.extend_from_slice(&12u16.to_be_bytes()); // Length = 12
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // XOR-MAPPED-ADDRESS
        response.extend_from_slice(&0x0020u16.to_be_bytes()); // Type
        response.extend_from_slice(&8u16.to_be_bytes()); // Length
        response.push(0x00);
        response.push(0x01); // IPv4
        response.extend_from_slice(&0u16.to_be_bytes()); // Port (don't care)
        response.extend_from_slice(&xor_addr.to_be_bytes());

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), Some(IpAddr::V4(ip)));
    }

    // ── XOR-MAPPED-ADDRESS (IPv6) ───────────────────────────────────

    #[test]
    fn test_xor_mapped_address_ipv6() {
        let tx_id: [u8; 12] = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C,
        ];
        let ip = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1);
        let ip_bytes = ip.octets();

        // Build XOR key = magic_cookie || transaction_id
        let mut xor_key = [0u8; 16];
        xor_key[0..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
        xor_key[4..16].copy_from_slice(&tx_id);

        let mut xored = [0u8; 16];
        for i in 0..16 {
            xored[i] = ip_bytes[i] ^ xor_key[i];
        }

        let mut response = Vec::with_capacity(44);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&24u16.to_be_bytes()); // Length = 24
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // XOR-MAPPED-ADDRESS
        response.extend_from_slice(&0x0020u16.to_be_bytes());
        response.extend_from_slice(&20u16.to_be_bytes()); // attr len = 20
        response.push(0x00);
        response.push(0x02); // IPv6
        response.extend_from_slice(&0u16.to_be_bytes()); // Port
        response.extend_from_slice(&xored);

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), Some(IpAddr::V6(ip)));
    }

    // ── MAPPED-ADDRESS (IPv4) — fallback ────────────────────────────

    #[test]
    fn test_mapped_address_ipv4_fallback() {
        let tx_id = [0u8; 12];
        let ip = Ipv4Addr::new(10, 0, 0, 1);

        let mut response = Vec::with_capacity(32);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&12u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // MAPPED-ADDRESS (0x0001)
        response.extend_from_slice(&0x0001u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.push(0x00);
        response.push(0x01); // IPv4
        response.extend_from_slice(&0u16.to_be_bytes()); // Port
        response.extend_from_slice(&ip.octets());

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), Some(IpAddr::V4(ip)));
    }

    #[test]
    fn test_mapped_address_ipv6_fallback() {
        let tx_id = [0u8; 12];
        let ip = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);

        let mut response = Vec::with_capacity(44);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&24u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // MAPPED-ADDRESS
        response.extend_from_slice(&0x0001u16.to_be_bytes());
        response.extend_from_slice(&20u16.to_be_bytes());
        response.push(0x00);
        response.push(0x02); // IPv6
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&ip.octets());

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), Some(IpAddr::V6(ip)));
    }

    // ── XOR-MAPPED-ADDRESS preferred over MAPPED-ADDRESS ────────────

    #[test]
    fn test_xor_mapped_preferred_over_mapped() {
        let tx_id = [0u8; 12];
        let mapped_ip = Ipv4Addr::new(1, 1, 1, 1);
        let xor_ip = Ipv4Addr::new(8, 8, 8, 8);
        let xor_addr = u32::from(xor_ip) ^ MAGIC_COOKIE;

        let mut response = Vec::with_capacity(48);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&24u16.to_be_bytes()); // 2 attrs × 12 bytes
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // MAPPED-ADDRESS first
        response.extend_from_slice(&0x0001u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.push(0x00);
        response.push(0x01);
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&mapped_ip.octets());
        // XOR-MAPPED-ADDRESS second
        response.extend_from_slice(&0x0020u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.push(0x00);
        response.push(0x01);
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&xor_addr.to_be_bytes());

        let msg = StunMessage::decode(&response).unwrap();
        // Should pick XOR-MAPPED-ADDRESS (8.8.8.8), not MAPPED-ADDRESS (1.1.1.1)
        assert_eq!(msg.get_mapped_address(), Some(IpAddr::V4(xor_ip)));
    }

    // ── No address attribute ────────────────────────────────────────

    #[test]
    fn test_no_mapped_address_returns_none() {
        let tx_id = [0u8; 12];

        let mut response = Vec::with_capacity(28);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // Unknown attribute (0x8000)
        response.extend_from_slice(&0x8000u16.to_be_bytes());
        response.extend_from_slice(&4u16.to_be_bytes());
        response.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), None);
    }

    // ── Attribute padding ───────────────────────────────────────────

    #[test]
    fn test_attribute_padding() {
        // Attribute with length=5 should be padded to 8 bytes
        let tx_id = [0u8; 12];

        let mut response = Vec::with_capacity(36);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&24u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // Unknown attr with odd length (5 bytes + 3 padding)
        response.extend_from_slice(&0x8001u16.to_be_bytes());
        response.extend_from_slice(&5u16.to_be_bytes());
        response.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05]);
        response.extend_from_slice(&[0x00, 0x00, 0x00]); // padding
                                                         // XOR-MAPPED-ADDRESS after padded attribute
        let xor_addr = u32::from(Ipv4Addr::new(172, 16, 0, 1)) ^ MAGIC_COOKIE;
        response.extend_from_slice(&0x0020u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.push(0x00);
        response.push(0x01);
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&xor_addr.to_be_bytes());

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(
            msg.get_mapped_address(),
            Some(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1)))
        );
    }

    // ── Invalid address family ──────────────────────────────────────

    #[test]
    fn test_xor_mapped_unknown_family() {
        let tx_id = [0u8; 12];

        let mut response = Vec::with_capacity(32);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&12u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // XOR-MAPPED-ADDRESS with unknown family 0x03
        response.extend_from_slice(&0x0020u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.push(0x00);
        response.push(0x03); // Unknown family
        response.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), None);
    }

    // ── Truncated attribute value ───────────────────────────────────

    #[test]
    fn test_xor_mapped_address_truncated_ipv4() {
        let tx_id = [0u8; 12];

        let mut response = Vec::with_capacity(28);
        response.extend_from_slice(&0x0101u16.to_be_bytes());
        response.extend_from_slice(&8u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);
        // XOR-MAPPED-ADDRESS with IPv4 family but only 4 bytes (need 8)
        response.extend_from_slice(&0x0020u16.to_be_bytes());
        response.extend_from_slice(&4u16.to_be_bytes()); // only 4 bytes
        response.push(0x00);
        response.push(0x01); // IPv4
        response.extend_from_slice(&0u16.to_be_bytes()); // Port only, no IP!

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.get_mapped_address(), None);
    }

    // ── Binding Error response type ─────────────────────────────────

    #[test]
    fn test_decode_error_response() {
        let tx_id = [0xAAu8; 12];

        let mut response = Vec::with_capacity(20);
        response.extend_from_slice(&0x0111u16.to_be_bytes()); // Binding Error
        response.extend_from_slice(&0u16.to_be_bytes());
        response.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        response.extend_from_slice(&tx_id);

        let msg = StunMessage::decode(&response).unwrap();
        assert_eq!(msg.msg_type, StunMethod::Error);
        assert_eq!(msg.transaction_id(), &tx_id);
    }
}
