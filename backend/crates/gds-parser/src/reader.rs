use std::io::{BufReader, Read};
use byteorder::{BigEndian, ReadBytesExt};

use crate::types::{
    GdsError, GdsRecord,
    RT_AREF, RT_ANGLE, RT_BGNLIB, RT_BGNSTR, RT_BOUNDARY, RT_BOX,
    RT_BOXTYPE, RT_COLROW, RT_DATATYPE, RT_ELFLAGS, RT_ENDEL, RT_ENDLIB,
    RT_ENDMASKS, RT_ENDSTR, RT_FORMAT, RT_GENERATIONS, RT_HEADER,
    RT_LAYER, RT_LIBNAME, RT_MAG, RT_MASK, RT_PATH, RT_PATHTYPE,
    RT_PLEX, RT_PRESENTATION, RT_PROPATTR, RT_PROPVALUE, RT_SNAME,
    RT_SREF, RT_STRANS, RT_STRNAME, RT_STRING, RT_TEXT, RT_TEXTTYPE,
    RT_UNITS, RT_WIDTH, RT_XY,
};

pub struct GdsReader<R: Read> {
    inner: R,
}

impl<R: Read> GdsReader<R> {
    pub fn new(reader: R) -> Self {
        GdsReader { inner: reader }
    }

    pub fn read_record(&mut self) -> Result<GdsRecord, GdsError> {
        let length = match self.inner.read_u16::<BigEndian>() {
            Ok(l) => l,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Err(GdsError::UnexpectedEof);
            }
            Err(e) => return Err(GdsError::IoError(e.to_string())),
        };

        if length < 4 {
            return Err(GdsError::RecordTooShort { length });
        }

        let record_type = self.inner.read_u8().map_err(|_| GdsError::TruncatedRecord)?;
        let _data_type  = self.inner.read_u8().map_err(|_| GdsError::TruncatedRecord)?;

        let data_len = (length - 4) as usize;
        let mut data = vec![0u8; data_len];
        if data_len > 0 {
            self.inner.read_exact(&mut data).map_err(|_| GdsError::TruncatedRecord)?;
        }

        let record = match record_type {
            RT_HEADER       => GdsRecord::Header(data),
            RT_BGNLIB       => GdsRecord::BgnLib(data),
            RT_LIBNAME      => GdsRecord::LibName(data),
            RT_UNITS        => GdsRecord::Units(data),
            RT_ENDLIB       => GdsRecord::EndLib,
            RT_BGNSTR       => GdsRecord::BgnStr(data),
            RT_STRNAME      => GdsRecord::StrName(data),
            RT_ENDSTR       => GdsRecord::EndStr,
            RT_BOUNDARY     => GdsRecord::Boundary,
            RT_PATH         => GdsRecord::Path,
            RT_SREF         => GdsRecord::Sref,
            RT_AREF         => GdsRecord::Aref,
            RT_TEXT         => GdsRecord::Text,
            RT_ENDEL        => GdsRecord::EndEl,
            RT_LAYER        => GdsRecord::Layer(data),
            RT_DATATYPE     => GdsRecord::DataType(data),
            RT_WIDTH        => GdsRecord::Width(data),
            RT_XY           => GdsRecord::XY(data),
            RT_SNAME        => GdsRecord::SName(data),
            RT_COLROW       => GdsRecord::ColRow(data),
            RT_STRANS       => GdsRecord::STrans(data),
            RT_MAG          => GdsRecord::Mag(data),
            RT_ANGLE        => GdsRecord::Angle(data),
            RT_TEXTTYPE     => GdsRecord::TextType(data),
            RT_PRESENTATION => GdsRecord::Presentation(data),
            RT_STRING       => GdsRecord::String(data),
            RT_PATHTYPE     => GdsRecord::PathType(data),
            RT_GENERATIONS  => GdsRecord::Generations(data),
            RT_ELFLAGS      => GdsRecord::ElFlags(data),
            RT_PROPATTR     => GdsRecord::PropAttr(data),
            RT_PROPVALUE    => GdsRecord::PropValue(data),
            RT_PLEX         => GdsRecord::Plex(data),
            RT_BOX          => GdsRecord::Box,
            RT_BOXTYPE      => GdsRecord::BoxType(data),
            RT_FORMAT       => GdsRecord::Format(data),
            RT_MASK         => GdsRecord::Mask(data),
            RT_ENDMASKS     => GdsRecord::EndMasks,
            unknown         => GdsRecord::Unknown { record_type: unknown, data },
        };

        Ok(record)
    }
}

impl GdsReader<BufReader<std::fs::File>> {
    pub fn from_path(path: impl AsRef<std::path::Path>) -> Result<Self, GdsError> {
        let file = std::fs::File::open(path)?;
        Ok(GdsReader::new(BufReader::new(file)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use crate::types::GdsRecord;

    fn make_record(record_type: u8, data_type: u8, data: &[u8]) -> Vec<u8> {
        let length = (4 + data.len()) as u16;
        let mut bytes = length.to_be_bytes().to_vec();
        bytes.push(record_type);
        bytes.push(data_type);
        bytes.extend_from_slice(data);
        bytes
    }

    #[test]
    fn reads_header_record() {
        let bytes = make_record(0x00, 0x02, &[0x00, 0x03]);
        let mut reader = GdsReader::new(Cursor::new(bytes));
        let record = reader.read_record().unwrap();
        if let GdsRecord::Header(data) = record {
            assert_eq!(data, vec![0x00, 0x03]);
        } else {
            panic!("expected Header");
        }
    }

    #[test]
    fn reads_endlib_as_no_data_variant() {
        let bytes = make_record(0x04, 0x00, &[]);
        let mut reader = GdsReader::new(Cursor::new(bytes));
        assert!(matches!(reader.read_record().unwrap(), GdsRecord::EndLib));
    }

    #[test]
    fn reads_strname_ascii_bytes() {
        let bytes = make_record(0x06, 0x06, b"TOP\0");
        let mut reader = GdsReader::new(Cursor::new(bytes));
        if let GdsRecord::StrName(data) = reader.read_record().unwrap() {
            assert_eq!(&data[..3], b"TOP");
        } else {
            panic!("expected StrName");
        }
    }

    #[test]
    fn reads_multiple_records_in_sequence() {
        let mut bytes = make_record(0x00, 0x02, &[0x00, 0x03]);
        bytes.extend(make_record(0x04, 0x00, &[]));
        let mut reader = GdsReader::new(Cursor::new(bytes));
        assert!(matches!(reader.read_record().unwrap(), GdsRecord::Header(_)));
        assert!(matches!(reader.read_record().unwrap(), GdsRecord::EndLib));
    }

    #[test]
    fn returns_unexpected_eof_on_empty_stream() {
        let mut reader = GdsReader::new(Cursor::new(vec![]));
        assert!(matches!(reader.read_record(), Err(GdsError::UnexpectedEof)));
    }

    #[test]
    fn returns_truncated_record_on_partial_data() {
        // length=8 but stream ends after the 4-byte header (no data bytes)
        let bytes = vec![0x00, 0x08, 0x00, 0x02];
        let mut reader = GdsReader::new(Cursor::new(bytes));
        assert!(matches!(reader.read_record(), Err(GdsError::TruncatedRecord)));
    }

    #[test]
    fn returns_record_too_short_for_length_under_4() {
        let bytes = vec![0x00, 0x02, 0x00, 0x00];
        let mut reader = GdsReader::new(Cursor::new(bytes));
        assert!(matches!(reader.read_record(), Err(GdsError::RecordTooShort { length: 2 })));
    }

    #[test]
    fn unknown_record_type_wraps_without_panic() {
        let bytes = make_record(0xFF, 0x00, &[]);
        let mut reader = GdsReader::new(Cursor::new(bytes));
        assert!(matches!(reader.read_record().unwrap(), GdsRecord::Unknown { record_type: 0xFF, .. }));
    }
}
