//! RFC-compliant fixed-size DNS types.
//!
//! This module provides strictly-sized types that conform to the DNS RFCs:
//! - RFC 1035: Domain Names - Implementation and Specification
//! - RFC 2181: Clarifications to the DNS Specification
//! - RFC 2782: A DNS RR for specifying the location of services (DNS SRV)
//!
//! All types in this module are designed to be:
//! - Fixed size (no heap allocation where possible)
//! - Copy where the size permits
//! - No Drop trait implementation required
//!
//! # Size Limits (from RFCs)
//!
//! | Field | Limit | Reference |
//! |-------|-------|-----------|
//! | Label | 1-63 octets | RFC 1035 §2.3.4 |
//! | Domain name | ≤255 octets | RFC 1035 §2.3.4 |
//! | TTL | 0 to 2^31-1 seconds | RFC 2181 §8 |
//! | TYPE | 16-bit unsigned | RFC 1035 §3.2.2 |
//! | CLASS | 16-bit unsigned | RFC 1035 §3.2.4 |
//! | Priority (MX/SRV) | 16-bit unsigned | RFC 1035 §3.3.9, RFC 2782 |
//! | Weight (SRV) | 16-bit unsigned | RFC 2782 |
//! | Port (SRV) | 16-bit unsigned | RFC 2782 |

use core::fmt;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Maximum length of a single DNS label (RFC 1035 §2.3.4).
pub const MAX_LABEL_LEN: usize = 63;

/// Maximum length of a full domain name including separators (RFC 1035 §2.3.4).
pub const MAX_DOMAIN_LEN: usize = 255;

/// Maximum TTL value per RFC 2181 §8: 2^31 - 1 seconds.
pub const MAX_TTL: u32 = 2_147_483_647;

/// API environment for providers that support sandbox/production modes.
///
/// Some DNS providers offer a sandbox environment for testing API integrations
/// without affecting production domains. This enum provides a standardized way
/// to specify the target environment.
///
/// # Example
///
/// ```
/// use libdns::types::Environment;
///
/// let env = Environment::Sandbox;
/// assert!(!env.is_production());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum Environment {
    /// Production environment - changes affect real domains.
    #[default]
    Production,
    /// Sandbox/testing environment - safe for development and testing.
    Sandbox,
}

impl Environment {
    /// Returns `true` if this is the production environment.
    pub fn is_production(&self) -> bool {
        matches!(self, Environment::Production)
    }

    /// Returns `true` if this is the sandbox environment.
    pub fn is_sandbox(&self) -> bool {
        matches!(self, Environment::Sandbox)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Environment::Production => write!(f, "production"),
            Environment::Sandbox => write!(f, "sandbox"),
        }
    }
}

/// A DNS label - a single component of a domain name.
///
/// Labels are limited to 63 octets (RFC 1035 §2.3.4).
/// The high-order two bits of the length octet must be zero.
///
/// This is a fixed-size, Copy type with no heap allocation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct Label {
    /// Length of the label (1-63).
    len: u8,
    /// Label data, padded to max size.
    data: [u8; MAX_LABEL_LEN],
}

impl Label {
    /// Creates a new label from a byte slice.
    ///
    /// Returns `None` if the slice is empty or exceeds 63 bytes.
    #[inline]
    pub const fn new(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() || bytes.len() > MAX_LABEL_LEN {
            return None;
        }

        let mut data = [0u8; MAX_LABEL_LEN];
        let mut i = 0;
        while i < bytes.len() {
            data[i] = bytes[i];
            i += 1;
        }

        Some(Self {
            len: bytes.len() as u8,
            data,
        })
    }

    /// Creates a new label from a string slice.
    ///
    /// Returns `None` if the string is empty or exceeds 63 bytes.
    #[inline]
    pub const fn from_str(s: &str) -> Option<Self> {
        Self::new(s.as_bytes())
    }

    /// Returns the label as a byte slice.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }

    /// Returns the length of the label in bytes.
    #[inline]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns true if the label is empty.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the label as a string slice, if valid UTF-8.
    #[inline]
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(self.as_bytes()).ok()
    }
}

impl fmt::Debug for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(s) => write!(f, "Label({:?})", s),
            None => write!(f, "Label({:?})", self.as_bytes()),
        }
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_str() {
            Some(s) => write!(f, "{}", s),
            None => write!(f, "{:?}", self.as_bytes()),
        }
    }
}

impl Default for Label {
    fn default() -> Self {
        Self {
            len: 0,
            data: [0u8; MAX_LABEL_LEN],
        }
    }
}

/// A DNS domain name with fixed-size storage.
///
/// Domain names are limited to 255 octets total (RFC 1035 §2.3.4).
/// This includes the length octets for each label and the terminating zero.
///
/// This is a fixed-size type with no heap allocation.
/// It's too large to be Copy (256 bytes), but implements Clone.
#[derive(Clone, PartialEq, Eq, Hash)]
#[repr(C)]
pub struct DomainName {
    /// Length of the domain name in wire format.
    len: u8,
    /// Domain name data in wire format (length-prefixed labels, null terminated).
    data: [u8; MAX_DOMAIN_LEN],
}

impl DomainName {
    /// Creates a new domain name from a dotted string (e.g., "example.com").
    ///
    /// Returns `None` if the domain name is invalid or too long.
    pub fn from_dotted(s: &str) -> Option<Self> {
        if s.is_empty() {
            // Root domain
            return Some(Self {
                len: 1,
                data: [0u8; MAX_DOMAIN_LEN],
            });
        }

        let mut data = [0u8; MAX_DOMAIN_LEN];
        let mut pos = 0usize;

        for label in s.trim_end_matches('.').split('.') {
            let label_bytes = label.as_bytes();
            if label_bytes.is_empty() || label_bytes.len() > MAX_LABEL_LEN {
                return None;
            }

            // Check if we have room: length byte + label + at least null terminator
            if pos + 1 + label_bytes.len() >= MAX_DOMAIN_LEN {
                return None;
            }

            data[pos] = label_bytes.len() as u8;
            pos += 1;

            data[pos..pos + label_bytes.len()].copy_from_slice(label_bytes);
            pos += label_bytes.len();
        }

        // Null terminator for root
        data[pos] = 0;
        pos += 1;

        Some(Self {
            len: pos as u8,
            data,
        })
    }

    /// Returns the domain name in dotted notation.
    pub fn to_dotted(&self) -> String {
        let mut result = String::with_capacity(self.len as usize);
        let mut pos = 0usize;

        while pos < self.len as usize {
            let label_len = self.data[pos] as usize;
            if label_len == 0 {
                break;
            }

            if !result.is_empty() {
                result.push('.');
            }

            pos += 1;
            if let Ok(s) = std::str::from_utf8(&self.data[pos..pos + label_len]) {
                result.push_str(s);
            }
            pos += label_len;
        }

        result
    }

    /// Returns the wire-format length of the domain name.
    #[inline]
    pub const fn wire_len(&self) -> usize {
        self.len as usize
    }

    /// Returns the wire-format bytes.
    #[inline]
    pub fn as_wire_bytes(&self) -> &[u8] {
        &self.data[..self.len as usize]
    }

    /// Returns true if this is the root domain.
    #[inline]
    pub const fn is_root(&self) -> bool {
        self.len == 1 && self.data[0] == 0
    }
}

impl fmt::Debug for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DomainName({:?})", self.to_dotted())
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_dotted())
    }
}

impl Default for DomainName {
    fn default() -> Self {
        Self {
            len: 1,
            data: [0u8; MAX_DOMAIN_LEN], // Root domain
        }
    }
}

/// DNS Time To Live value.
///
/// Per RFC 2181 §8, TTL is an unsigned 32-bit integer with a maximum
/// value of 2^31 - 1 (2,147,483,647) seconds. The MSB must be zero.
///
/// This type enforces the RFC limit and provides semantic clarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct Ttl(u32);

impl Ttl {
    /// Zero TTL - record should not be cached (RFC 1035 §3.2.1).
    pub const ZERO: Ttl = Ttl(0);

    /// One hour TTL.
    pub const ONE_HOUR: Ttl = Ttl(3600);

    /// One day TTL.
    pub const ONE_DAY: Ttl = Ttl(86400);

    /// One week TTL.
    pub const ONE_WEEK: Ttl = Ttl(604800);

    /// Maximum valid TTL per RFC 2181 §8.
    pub const MAX: Ttl = Ttl(MAX_TTL);

    /// Creates a new TTL, clamping to the RFC maximum if necessary.
    #[inline]
    pub const fn new(seconds: u32) -> Self {
        if seconds > MAX_TTL {
            Self(MAX_TTL)
        } else {
            Self(seconds)
        }
    }

    /// Creates a TTL from seconds, returning None if it exceeds the RFC maximum.
    #[inline]
    pub const fn try_new(seconds: u32) -> Option<Self> {
        if seconds > MAX_TTL {
            None
        } else {
            Some(Self(seconds))
        }
    }

    /// Returns the TTL value in seconds.
    #[inline]
    pub const fn as_secs(&self) -> u32 {
        self.0
    }

    /// Returns true if the TTL is zero.
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }
}

impl From<u32> for Ttl {
    #[inline]
    fn from(secs: u32) -> Self {
        Self::new(secs)
    }
}

impl From<Ttl> for u32 {
    #[inline]
    fn from(ttl: Ttl) -> Self {
        ttl.0
    }
}

impl fmt::Display for Ttl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// DNS Record Type (RFC 1035 §3.2.2).
///
/// This is a 16-bit unsigned integer representing the record type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u16)]
pub enum RecordType {
    /// Host address (RFC 1035).
    A = 1,
    /// Authoritative name server (RFC 1035).
    NS = 2,
    /// Canonical name for an alias (RFC 1035).
    CNAME = 5,
    /// Start of authority (RFC 1035).
    SOA = 6,
    /// Domain name pointer (RFC 1035).
    PTR = 12,
    /// Host information (RFC 1035).
    HINFO = 13,
    /// Mail exchange (RFC 1035).
    MX = 15,
    /// Text strings (RFC 1035).
    TXT = 16,
    /// IPv6 host address (RFC 3596).
    AAAA = 28,
    /// Server selection (RFC 2782).
    SRV = 33,
    /// Delegation signer (RFC 4034).
    DS = 43,
    /// DNSKEY (RFC 4034).
    DNSKEY = 48,
    /// Certification Authority Authorization (RFC 8659).
    CAA = 257,
}

impl RecordType {
    /// Creates a RecordType from a u16 value.
    pub const fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::A),
            2 => Some(Self::NS),
            5 => Some(Self::CNAME),
            6 => Some(Self::SOA),
            12 => Some(Self::PTR),
            13 => Some(Self::HINFO),
            15 => Some(Self::MX),
            16 => Some(Self::TXT),
            28 => Some(Self::AAAA),
            33 => Some(Self::SRV),
            43 => Some(Self::DS),
            48 => Some(Self::DNSKEY),
            257 => Some(Self::CAA),
            _ => None,
        }
    }

    /// Creates a RecordType from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "A" => Some(Self::A),
            "NS" => Some(Self::NS),
            "CNAME" => Some(Self::CNAME),
            "SOA" => Some(Self::SOA),
            "PTR" => Some(Self::PTR),
            "HINFO" => Some(Self::HINFO),
            "MX" => Some(Self::MX),
            "TXT" => Some(Self::TXT),
            "AAAA" => Some(Self::AAAA),
            "SRV" => Some(Self::SRV),
            "DS" => Some(Self::DS),
            "DNSKEY" => Some(Self::DNSKEY),
            "CAA" => Some(Self::CAA),
            _ => None,
        }
    }

    /// Returns the type code as a u16.
    #[inline]
    pub const fn as_u16(&self) -> u16 {
        *self as u16
    }

    /// Returns the type as a string.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::A => "A",
            Self::NS => "NS",
            Self::CNAME => "CNAME",
            Self::SOA => "SOA",
            Self::PTR => "PTR",
            Self::HINFO => "HINFO",
            Self::MX => "MX",
            Self::TXT => "TXT",
            Self::AAAA => "AAAA",
            Self::SRV => "SRV",
            Self::DS => "DS",
            Self::DNSKEY => "DNSKEY",
            Self::CAA => "CAA",
        }
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// DNS Record Class (RFC 1035 §3.2.4).
///
/// This is a 16-bit unsigned integer representing the record class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[repr(u16)]
pub enum RecordClass {
    /// The Internet (RFC 1035).
    #[default]
    IN = 1,
    /// CSNET class (obsolete, RFC 1035).
    CS = 2,
    /// CHAOS class (RFC 1035).
    CH = 3,
    /// Hesiod (RFC 1035).
    HS = 4,
}

impl RecordClass {
    /// Creates a RecordClass from a u16 value.
    pub const fn from_u16(value: u16) -> Option<Self> {
        match value {
            1 => Some(Self::IN),
            2 => Some(Self::CS),
            3 => Some(Self::CH),
            4 => Some(Self::HS),
            _ => None,
        }
    }

    /// Returns the class code as a u16.
    #[inline]
    pub const fn as_u16(&self) -> u16 {
        *self as u16
    }
}

impl fmt::Display for RecordClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IN => write!(f, "IN"),
            Self::CS => write!(f, "CS"),
            Self::CH => write!(f, "CH"),
            Self::HS => write!(f, "HS"),
        }
    }
}

/// MX record data with fixed-size priority (RFC 1035 §3.3.9).
///
/// Priority is a 16-bit unsigned integer. Lower values are preferred.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MxData {
    /// Preference value (lower is more preferred).
    pub priority: u16,
    /// Mail exchange host.
    pub exchange: DomainName,
}

impl MxData {
    /// Creates new MX data.
    pub fn new(priority: u16, exchange: DomainName) -> Self {
        Self { priority, exchange }
    }
}

impl fmt::Debug for MxData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MxData")
            .field("priority", &self.priority)
            .field("exchange", &self.exchange.to_dotted())
            .finish()
    }
}

/// SRV record data with fixed-size fields (RFC 2782).
///
/// All numeric fields are 16-bit unsigned integers.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SrvData {
    /// Priority (lower is more preferred).
    pub priority: u16,
    /// Weight for load balancing among same-priority servers.
    pub weight: u16,
    /// TCP/UDP port number.
    pub port: u16,
    /// Target host providing the service.
    pub target: DomainName,
}

impl SrvData {
    /// Creates new SRV data.
    pub fn new(priority: u16, weight: u16, port: u16, target: DomainName) -> Self {
        Self {
            priority,
            weight,
            port,
            target,
        }
    }
}

impl fmt::Debug for SrvData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SrvData")
            .field("priority", &self.priority)
            .field("weight", &self.weight)
            .field("port", &self.port)
            .field("target", &self.target.to_dotted())
            .finish()
    }
}

/// SOA record data (RFC 1035 §3.3.13).
///
/// All timing values are 32-bit unsigned integers representing seconds.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SoaData {
    /// Primary name server for the zone.
    pub mname: DomainName,
    /// Email of the responsible person (encoded as domain name).
    pub rname: DomainName,
    /// Serial number (zone version).
    pub serial: u32,
    /// Refresh interval in seconds.
    pub refresh: u32,
    /// Retry interval in seconds.
    pub retry: u32,
    /// Expire time in seconds.
    pub expire: u32,
    /// Minimum TTL in seconds.
    pub minimum: u32,
}

impl fmt::Debug for SoaData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SoaData")
            .field("mname", &self.mname.to_dotted())
            .field("rname", &self.rname.to_dotted())
            .field("serial", &self.serial)
            .field("refresh", &self.refresh)
            .field("retry", &self.retry)
            .field("expire", &self.expire)
            .field("minimum", &self.minimum)
            .finish()
    }
}
