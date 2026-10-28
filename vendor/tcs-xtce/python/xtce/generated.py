# @generated automatically by tcs-xtce (generator/xtcegen) from
# references/SpaceSystem.xsd.
# Any changes made to this file will be lost when it is regenerated;
# change generator/xtcegen instead and run `make generate`.
# tcs-xtce: https://github.com/vomlehn/tcs-xtce
# Rust crate: https://crates.io/crates/tcs-xtce
"""Data structures for XTCE 1.3 (``http://www.omg.org/spec/XTCE/20250214``)."""

from __future__ import annotations

import enum
from dataclasses import dataclass, field
from typing import Any, ClassVar, Optional

from . import _xml as X

NAMESPACE = 'http://www.omg.org/spec/XTCE/20250214'
SCHEMA_VERSION = '1.3'
ROOT_ELEMENT = 'SpaceSystem'
ROOT_TYPE_NAME = 'SpaceSystemType'



class BasisType(str, enum.Enum):
    """Defines to type of update rates: perSecond and perContainerUpdate. See
    RateInStreamType."""

    PER_SECOND = 'perSecond'
    PER_CONTAINER_UPDATE = 'perContainerUpdate'


_E_BasisType = {m.value.strip(): m for m in BasisType}


def _p_BasisType(s: str) -> BasisType:
    try:
        return _E_BasisType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid BasisType {s!r}") from None


class BitOrderType(str, enum.Enum):
    """Defines two bit-order types: most significant bit first and least
    significant bit first. See DataEncodingType."""

    LEAST_SIGNIFICANT_BIT_FIRST = 'leastSignificantBitFirst'
    MOST_SIGNIFICANT_BIT_FIRST = 'mostSignificantBitFirst'


_E_BitOrderType = {m.value.strip(): m for m in BitOrderType}


def _p_BitOrderType(s: str) -> BitOrderType:
    try:
        return _E_BitOrderType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid BitOrderType {s!r}") from None


class ByteOrderCommonType(str, enum.Enum):
    """Common byte orderings: most significant byte first (also known as big
    endian) and least significant byte first (also known as little endian)."""

    MOST_SIGNIFICANT_BYTE_FIRST = 'mostSignificantByteFirst'
    LEAST_SIGNIFICANT_BYTE_FIRST = 'leastSignificantByteFirst'


_E_ByteOrderCommonType = {m.value.strip(): m for m in ByteOrderCommonType}


def _p_ByteOrderCommonType(s: str) -> ByteOrderCommonType:
    try:
        return _E_ByteOrderCommonType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ByteOrderCommonType {s!r}") from None


class ChangeBasisType(str, enum.Enum):
    """Defines absoluteChange and percentageChange for use in rate of change
    alarms. Used by ChangeAlarmRangesType."""

    ABSOLUTE_CHANGE = 'absoluteChange'
    PERCENTAGE_CHANGE = 'percentageChange'


_E_ChangeBasisType = {m.value.strip(): m for m in ChangeBasisType}


def _p_ChangeBasisType(s: str) -> ChangeBasisType:
    try:
        return _E_ChangeBasisType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ChangeBasisType {s!r}") from None


class ChangeSpanType(str, enum.Enum):
    """Defines a changePerSecond and changePerSample for use in rate of change
    alarms. Used by ChangeAlarmRangesType."""

    CHANGE_PER_SECOND = 'changePerSecond'
    CHANGE_PER_SAMPLE = 'changePerSample'


_E_ChangeSpanType = {m.value.strip(): m for m in ChangeSpanType}


def _p_ChangeSpanType(s: str) -> ChangeSpanType:
    try:
        return _E_ChangeSpanType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ChangeSpanType {s!r}") from None


class CharacterWidthType(str, enum.Enum):
    """XTCE enumeration."""

    V8 = '8'
    V16 = '16'
    V32 = '32'


_E_CharacterWidthType = {m.value.strip(): m for m in CharacterWidthType}


def _p_CharacterWidthType(s: str) -> CharacterWidthType:
    try:
        return _E_CharacterWidthType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid CharacterWidthType {s!r}") from None


class ChecksumnameType(str, enum.Enum):
    """XTCE enumeration."""

    UNIX_SUM = 'unix_sum'
    SUM8 = 'sum8'
    SUM16 = 'sum16'
    SUM24 = 'sum24'
    SUM32 = 'sum32'
    FLETCHER4 = 'fletcher4'
    FLETCHER8 = 'fletcher8'
    FLETCHER16 = 'fletcher16'
    FLETCHER32 = 'fletcher32'
    ADLER32 = 'adler32'
    LUHN = 'luhn'
    VERHOEFF = 'verhoeff'
    DAMM = 'damm'
    CUSTOM = 'custom'


_E_ChecksumnameType = {m.value.strip(): m for m in ChecksumnameType}


def _p_ChecksumnameType(s: str) -> ChecksumnameType:
    try:
        return _E_ChecksumnameType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ChecksumnameType {s!r}") from None


class ComparisonOperatorsType(str, enum.Enum):
    """Operators to use when testing a boolean condition for a validity check"""

    EMPTY = '=='
    EMPTY_2 = '!='
    EMPTY_3 = '<'
    EMPTY_4 = '<='
    EMPTY_5 = '>'
    EMPTY_6 = '>='


_E_ComparisonOperatorsType = {m.value.strip(): m
                              for m in ComparisonOperatorsType}


def _p_ComparisonOperatorsType(s: str) -> ComparisonOperatorsType:
    try:
        return _E_ComparisonOperatorsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ComparisonOperatorsType {s!r}") from None


class ConcernLevelsType(str, enum.Enum):
    """Defines six levels: Normal, Watch, Warning, Distress, Critical and
    Severe, in that order of concern from least to most. These level definitions
    are used throughout the alarm definitions. An implementation should
    interpret these as best to match their uniqueness and provide documentation
    on how this standard maps to their implementation. Not all are likely to be
    provided, with some either ignored, promoted or demoted to others, or warned
    on input. There exist some reasonable usage recommendations in the user
    community."""

    NORMAL = 'normal'
    WATCH = 'watch'
    WARNING = 'warning'
    DISTRESS = 'distress'
    CRITICAL = 'critical'
    SEVERE = 'severe'


_E_ConcernLevelsType = {m.value.strip(): m for m in ConcernLevelsType}


def _p_ConcernLevelsType(s: str) -> ConcernLevelsType:
    try:
        return _E_ConcernLevelsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ConcernLevelsType {s!r}") from None


class ConsequenceLevelType(str, enum.Enum):
    """Defines the criticality level of a command. Criticality levels follow ISO
    14950."""

    NORMAL = 'normal'
    VITAL = 'vital'
    CRITICAL = 'critical'
    FORBIDDEN = 'forbidden'
    USER1 = 'user1'
    USER2 = 'user2'


_E_ConsequenceLevelType = {m.value.strip(): m for m in ConsequenceLevelType}


def _p_ConsequenceLevelType(s: str) -> ConsequenceLevelType:
    try:
        return _E_ConsequenceLevelType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ConsequenceLevelType {s!r}") from None


class EpochTimeEnumsType(str, enum.Enum):
    """Union values of common epoch definitions for document convenience."""

    TAI = 'TAI'
    J2000 = 'J2000'
    UNIX = 'UNIX'
    GPS = 'GPS'


_E_EpochTimeEnumsType = {m.value.strip(): m for m in EpochTimeEnumsType}


def _p_EpochTimeEnumsType(s: str) -> EpochTimeEnumsType:
    try:
        return _E_EpochTimeEnumsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid EpochTimeEnumsType {s!r}") from None


class FlagBitType(str, enum.Enum):
    """XTCE enumeration."""

    ZEROS = 'zeros'
    ONES = 'ones'


_E_FlagBitType = {m.value.strip(): m for m in FlagBitType}


def _p_FlagBitType(s: str) -> FlagBitType:
    try:
        return _E_FlagBitType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid FlagBitType {s!r}") from None


class FloatEncodingSizeInBitsType(str, enum.Enum):
    """XTCE enumeration."""

    V16 = '16'
    V32 = '32'
    V40 = '40'
    V48 = '48'
    V64 = '64'
    V80 = '80'
    V128 = '128'


_E_FloatEncodingSizeInBitsType = {m.value.strip(): m
                                  for m in FloatEncodingSizeInBitsType}


def _p_FloatEncodingSizeInBitsType(s: str) -> FloatEncodingSizeInBitsType:
    try:
        return _E_FloatEncodingSizeInBitsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid FloatEncodingSizeInBitsType {s!r}") from None


class FloatEncodingType(str, enum.Enum):
    """XTCE enumeration."""

    IEEE754_1985 = 'IEEE754_1985'
    IEEE754 = 'IEEE754'
    MILSTD_1750_A = 'MILSTD_1750A'
    DEC = 'DEC'
    IBM = 'IBM'
    TI = 'TI'


_E_FloatEncodingType = {m.value.strip(): m for m in FloatEncodingType}


def _p_FloatEncodingType(s: str) -> FloatEncodingType:
    try:
        return _E_FloatEncodingType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid FloatEncodingType {s!r}") from None


class FloatSizeInBitsType(str, enum.Enum):
    """XTCE enumeration."""

    V32 = '32'
    V64 = '64'
    V128 = '128'


_E_FloatSizeInBitsType = {m.value.strip(): m for m in FloatSizeInBitsType}


def _p_FloatSizeInBitsType(s: str) -> FloatSizeInBitsType:
    try:
        return _E_FloatSizeInBitsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid FloatSizeInBitsType {s!r}") from None


class FloatingPointNotationType(str, enum.Enum):
    """XTCE enumeration."""

    NORMAL = 'normal'
    SCIENTIFIC = 'scientific'
    ENGINEERING = 'engineering'


_E_FloatingPointNotationType = {m.value.strip(): m
                                for m in FloatingPointNotationType}


def _p_FloatingPointNotationType(s: str) -> FloatingPointNotationType:
    try:
        return _E_FloatingPointNotationType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid FloatingPointNotationType {s!r}") from None


class IntegerEncodingType(str, enum.Enum):
    """XTCE enumeration."""

    UNSIGNED = 'unsigned'
    SIGN_MAGNITUDE = 'signMagnitude'
    TWOS_COMPLEMENT = 'twosComplement'
    ONES_COMPLEMENT = 'onesComplement'
    BCD = 'BCD'
    PACKED_BCD = 'packedBCD'


_E_IntegerEncodingType = {m.value.strip(): m for m in IntegerEncodingType}


def _p_IntegerEncodingType(s: str) -> IntegerEncodingType:
    try:
        return _E_IntegerEncodingType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid IntegerEncodingType {s!r}") from None


class MathOperatorsType(str, enum.Enum):
    """Mathematical operators used in the math operation. Behavior of each
    operator on the stack is described using notation (before -- after), where
    "before" represents the stack before execution of the operator and "after"
    represent the stack after execution."""

    EMPTY = '+'
    EMPTY_2 = '-'
    EMPTY_3 = '*'
    EMPTY_4 = '/'
    EMPTY_5 = '%'
    EMPTY_6 = '^'
    YX = 'y^x'
    LN = 'ln'
    LOG = 'log'
    EX = 'e^x'
    V1_X = '1/x'
    X = 'x!'
    TAN = 'tan'
    COS = 'cos'
    SIN = 'sin'
    ATAN = 'atan'
    ATAN2 = 'atan2'
    ACOS = 'acos'
    ASIN = 'asin'
    TANH = 'tanh'
    COSH = 'cosh'
    SINH = 'sinh'
    ATANH = 'atanh'
    ACOSH = 'acosh'
    ASINH = 'asinh'
    SWAP = 'swap'
    DROP = 'drop'
    DUP = 'dup'
    OVER = 'over'
    EMPTY_7 = '<<'
    EMPTY_8 = '>>'
    EMPTY_9 = '&'
    EMPTY_10 = '|'
    EMPTY_11 = '&&'
    EMPTY_12 = '||'
    EMPTY_13 = '!'
    ABS = 'abs'
    DIV = 'div'
    INT = 'int'
    EMPTY_14 = '>'
    EMPTY_15 = '>='
    EMPTY_16 = '<'
    EMPTY_17 = '<='
    EMPTY_18 = '=='
    EMPTY_19 = '!='
    MIN = 'min'
    MAX = 'max'
    XOR = 'xor'
    EMPTY_20 = '~'


_E_MathOperatorsType = {m.value.strip(): m for m in MathOperatorsType}


def _p_MathOperatorsType(s: str) -> MathOperatorsType:
    try:
        return _E_MathOperatorsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid MathOperatorsType {s!r}") from None


class PCMType(str, enum.Enum):
    """XTCE enumeration."""

    NRZL = 'NRZL'
    NRZM = 'NRZM'
    NRZS = 'NRZS'
    BI_PHASE_L = 'BiPhaseL'
    BI_PHASE_M = 'BiPhaseM'
    BI_PHASE_S = 'BiPhaseS'


_E_PCMType = {m.value.strip(): m for m in PCMType}


def _p_PCMType(s: str) -> PCMType:
    try:
        return _E_PCMType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid PCMType {s!r}") from None


class ParityFormType(str, enum.Enum):
    """XTCE enumeration."""

    EVEN = 'Even'
    ODD = 'Odd'


_E_ParityFormType = {m.value.strip(): m for m in ParityFormType}


def _p_ParityFormType(s: str) -> ParityFormType:
    try:
        return _E_ParityFormType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ParityFormType {s!r}") from None


class RadixType(str, enum.Enum):
    """Specifies the number base"""

    DECIMAL = 'Decimal'
    HEXADECIMAL = 'Hexadecimal'
    OCTAL = 'Octal'
    BINARY = 'Binary'


_E_RadixType = {m.value.strip(): m for m in RadixType}


def _p_RadixType(s: str) -> RadixType:
    try:
        return _E_RadixType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid RadixType {s!r}") from None


class RangeFormType(str, enum.Enum):
    """Defines inside and outside enumerated terms, where the term outside means
    the range is (-inf, minimum) and (maximum, inf) -- that is a range where
    acceptable values must be less than the minimum and greater than the
    maximum, and the term inside means the range is (minimum, maximum) -- that
    is acceptable values are between the minimum and maximum (either the min or
    max may be inclusive or exclusive)."""

    OUTSIDE = 'outside'
    INSIDE = 'inside'


_E_RangeFormType = {m.value.strip(): m for m in RangeFormType}


def _p_RangeFormType(s: str) -> RangeFormType:
    try:
        return _E_RangeFormType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid RangeFormType {s!r}") from None


class ReferenceLocationType(str, enum.Enum):
    """The location may be relative to the start of the container
    (containerStart), relative to the end of the previous entry (previousEntry),
    relative to the end of the container (containerEnd), or relative to the
    entry that follows this one (nextEntry). If going forward (containerStart
    and previousEntry) then the location refers to the start of the Entry. If
    going backwards (containerEnd and nextEntry) then, the location refers to
    the end of the entry."""

    CONTAINER_START = 'containerStart'
    CONTAINER_END = 'containerEnd'
    PREVIOUS_ENTRY = 'previousEntry'
    NEXT_ENTRY = 'nextEntry'


_E_ReferenceLocationType = {m.value.strip(): m for m in ReferenceLocationType}


def _p_ReferenceLocationType(s: str) -> ReferenceLocationType:
    try:
        return _E_ReferenceLocationType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ReferenceLocationType {s!r}") from None


class ReferencePointType(str, enum.Enum):
    """XTCE enumeration."""

    START = 'start'
    END = 'end'


_E_ReferencePointType = {m.value.strip(): m for m in ReferencePointType}


def _p_ReferencePointType(s: str) -> ReferencePointType:
    try:
        return _E_ReferencePointType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ReferencePointType {s!r}") from None


class StringEncodingType(str, enum.Enum):
    """Defines string encodings. US-ASCII (7-bit), ISO-8859-1 (8-bit Extended
    ASCII), Windows-1252 (8-bit Extended ASCII), UTF-8 (Unicode), UTF-16
    (Unicode with Byte Order Mark), UTF-16LE (Unicode Little Endian), UTF-16BE
    (Unicode Big Endian). See StringDataEncodingType."""

    USASCII = 'US-ASCII'
    ISO_8859_1 = 'ISO-8859-1'
    WINDOWS_1252 = 'Windows-1252'
    UTF_8 = 'UTF-8'
    UTF_16 = 'UTF-16'
    UTF_16_LE = 'UTF-16LE'
    UTF_16_BE = 'UTF-16BE'
    UTF_32 = 'UTF-32'
    UTF_32_LE = 'UTF-32LE'
    UTF_32_BE = 'UTF-32BE'


_E_StringEncodingType = {m.value.strip(): m for m in StringEncodingType}


def _p_StringEncodingType(s: str) -> StringEncodingType:
    try:
        return _E_StringEncodingType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid StringEncodingType {s!r}") from None


class SystemTypeType(str, enum.Enum):
    """The type attribute represents what from a space enterprise this
    SpaceSystem element represents. See the enumerations for specific details.
    Unknown is the default for backwards compatibility, though it should be
    avoided in newer documents."""

    ASSET = 'asset'
    ASSET_GROUP = 'assetGroup'
    ASSET_COMPONENT = 'assetComponent'
    UNKNOWN = 'unknown'


_E_SystemTypeType = {m.value.strip(): m for m in SystemTypeType}


def _p_SystemTypeType(s: str) -> SystemTypeType:
    try:
        return _E_SystemTypeType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid SystemTypeType {s!r}") from None


class TelemetryDataSourceType(str, enum.Enum):
    """A telemetered Parameter is one that will have values in telemetry. A
    derived Parameter is one that is calculated, usually by an Algorithm. A
    constant Parameter is one that is used as a constant in the system (e.g. a
    vehicle id). A local Parameter is one that is used purely by the software
    locally (e.g. a ground command counter). A ground Parameter is one that is
    generated by an asset which is not the spacecraft."""

    TELEMETERED = 'telemetered'
    DERIVED = 'derived'
    CONSTANT = 'constant'
    LOCAL = 'local'
    GROUND = 'ground'


_E_TelemetryDataSourceType = {m.value.strip(): m
                              for m in TelemetryDataSourceType}


def _p_TelemetryDataSourceType(s: str) -> TelemetryDataSourceType:
    try:
        return _E_TelemetryDataSourceType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid TelemetryDataSourceType {s!r}") from None


class TimeAssociationUnitType(str, enum.Enum):
    """Time units the time association decimal value is in."""

    SECONDS = 'seconds'
    MILLISECONDS = 'milliseconds'
    MICROSECONDS = 'microseconds'
    NANOSECONDS = 'nanoseconds'
    PICOSECONDS = 'picoseconds'
    MINUTES = 'minutes'
    HOURS = 'hours'
    DAYS = 'days'
    MONTHS = 'months'
    YEARS = 'years'


_E_TimeAssociationUnitType = {m.value.strip(): m
                              for m in TimeAssociationUnitType}


def _p_TimeAssociationUnitType(s: str) -> TimeAssociationUnitType:
    try:
        return _E_TimeAssociationUnitType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid TimeAssociationUnitType {s!r}") from None


class TimeUnitsType(str, enum.Enum):
    """Base time unit of measure. It is best practice to avoid days, months, and
    years due to ambiguity involving leap seconds and leap days. If these are
    used, the system should document how the leaps are handled."""

    SECONDS = 'seconds'
    MILLISECONDS = 'milliseconds'
    MICROSECONDS = 'microseconds'
    NANOSECONDS = 'nanoseconds'
    PICOSECONDS = 'picoseconds'
    MINUTES = 'minutes'
    HOURS = 'hours'
    DAYS = 'days'
    MONTHS = 'months'
    YEARS = 'years'


_E_TimeUnitsType = {m.value.strip(): m for m in TimeUnitsType}


def _p_TimeUnitsType(s: str) -> TimeUnitsType:
    try:
        return _E_TimeUnitsType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid TimeUnitsType {s!r}") from None


class TimeWindowIsRelativeToType(str, enum.Enum):
    """XTCE enumeration."""

    COMMAND_RELEASE = 'commandRelease'
    TIME_LAST_VERIFIER_PASSED = 'timeLastVerifierPassed'


_E_TimeWindowIsRelativeToType = {m.value.strip(): m
                                 for m in TimeWindowIsRelativeToType}


def _p_TimeWindowIsRelativeToType(s: str) -> TimeWindowIsRelativeToType:
    try:
        return _E_TimeWindowIsRelativeToType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid TimeWindowIsRelativeToType {s!r}") from None


class UnitFormType(str, enum.Enum):
    """Defines enumerated values to categorize a unit associated with a
    telemetered value. Typically the unit refers to the calibrated (engineering)
    value. In some cases the unit may be associated with the uncalibrated or raw
    values. Uncalibrated and raw here are typically synonymous, but there are
    exceptions."""

    CALIBRATED = 'calibrated'
    UNCALIBRATED = 'uncalibrated'
    RAW = 'raw'


_E_UnitFormType = {m.value.strip(): m for m in UnitFormType}


def _p_UnitFormType(s: str) -> UnitFormType:
    try:
        return _E_UnitFormType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid UnitFormType {s!r}") from None


class ValidationStatusType(str, enum.Enum):
    """XTCE enumeration."""

    UNKNOWN = 'Unknown'
    WORKING = 'Working'
    DRAFT = 'Draft'
    TEST = 'Test'
    VALIDATED = 'Validated'
    RELEASED = 'Released'
    WITHDRAWN = 'Withdrawn'


_E_ValidationStatusType = {m.value.strip(): m for m in ValidationStatusType}


def _p_ValidationStatusType(s: str) -> ValidationStatusType:
    try:
        return _E_ValidationStatusType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid ValidationStatusType {s!r}") from None


class VerifierEnumerationType(str, enum.Enum):
    """An enumerated list of verifier types"""

    RELEASE = 'release'
    TRANSFERRED_TO_RANGE = 'transferredToRange'
    SENT_FROM_RANGE = 'sentFromRange'
    RECEIVED = 'received'
    ACCEPTED = 'accepted'
    QUEUED = 'queued'
    EXECUTING = 'executing'
    COMPLETE = 'complete'
    FAILED = 'failed'


_E_VerifierEnumerationType = {m.value.strip(): m
                              for m in VerifierEnumerationType}


def _p_VerifierEnumerationType(s: str) -> VerifierEnumerationType:
    try:
        return _E_VerifierEnumerationType[s.strip()]
    except KeyError:
        raise ValueError(f"invalid VerifierEnumerationType {s!r}") from None


@dataclass(slots=True)
class ANDedConditionsType:
    """Describe two or more conditions that are logically anded together.
    Conditions may be a mix of Condition and ORedCondition. See
    ORedConditionType and BooleanExpressionType."""

    items: list[ANDedConditionsChoice] = field(default_factory=list)


@dataclass(slots=True)
class AbsoluteTimeArgumentType:
    """Describe an absolute time argument type relative to a known epoch (such
    as TAI). The string representation of this time should use the [ISO 8601]
    extended format CCYY-MM-DDThh:mm:ss where "CC" represents the century, "YY"
    the year, "MM" the month and "DD" the day, preceded by an optional leading
    "-" sign to indicate a negative number. If the sign is omitted, "+" is
    assumed. The letter "T" is the date/time separator and "hh", "mm", "ss"
    represent hour, minute and second respectively. Additional digits can be
    used to increase the precision of fractional seconds if desired i.e. the
    format ss.ss... with any number of digits after the decimal point is
    supported. See TAIType, IntegerDataEncoding and AbsoluteTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class AbsoluteTimeDataType:
    """A base schema type for describing an absolute time data type. Contains an
    absolute (to a known epoch) time. Use the [ISO 8601] extended format
    CCYY-MM-DDThh:mm:ss where "CC" represents the century, "YY" the year, "MM"
    the month and "DD" the day, preceded by an optional leading "-" sign to
    indicate a negative number. If the sign is omitted, "+" is assumed. The
    letter "T" is the date/time separator and "hh", "mm", "ss" represent hour,
    minute and second respectively. Additional digits can be used to increase
    the precision of fractional seconds if desired i.e. the format ss.ss... with
    any number of digits after the decimal point is supported. See
    AbsoluteTimeParameterType and AbsoluteTimeArgumentType. See
    AbsouteTimeParameterType, AbsoluteTimeArgumentType and BaseTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class AbsoluteTimeParameterType:
    """Describe an absolute time parameter type relative to a known epoch (such
    as TAI). The string representation of this time should use the [ISO 8601]
    extended format CCYY-MM-DDThh:mm:ss where "CC" represents the century, "YY"
    the year, "MM" the month and "DD" the day, preceded by an optional leading
    "-" sign to indicate a negative number. If the sign is omitted, "+" is
    assumed. The letter "T" is the date/time separator and "hh", "mm", "ss"
    represent hour, minute and second respectively. Additional digits can be
    used to increase the precision of fractional seconds if desired i.e. the
    format ss.ss... with any number of digits after the decimal point is
    supported. See TAIType, IntegerDataEncoding and AbsoluteTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class AcceptedVerifierType:
    """A verifier that means the destination has accepted the command."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: AcceptedVerifierChoice1 = field(
        default_factory=lambda: AcceptedVerifierChoice1())
    choice2: AcceptedVerifierChoice2 = field(
        default_factory=lambda: AcceptedVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class AggregateArgumentType:
    """Describe a complex data type analogous to a C-struct. Each field of the
    data type is called a Member. Each Member is part of the MemberList which
    forms the list of items to be placed under this data type's name. The
    MemberList defines a data block and block's size is defined by the
    DataEncodings of each Member's type reference. The data members are ordered
    and contiguous in the MemberList element (packed). Each member may be
    addressed by the dot syntax similar to C such as P.voltage if P is the
    referring parameter and voltage is of a member of P's aggregate type. See
    MemberType, MemberListType, DataEncodingType, NameReferenceType, and
    AggregateDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'MemberList'
    member_list: MemberListType = field(
        default_factory=lambda: MemberListType())


@dataclass(slots=True)
class AggregateDataType:
    """A base schema type for describing a complex data type analogous to a
    C-struct. Each field of the data type is called a Member. Each Member is
    part of the MemberList which forms the list of items to be placed under this
    data type's name. The MemberList defines a data block and block's size is
    defined by the DataEncodings of each Member's type reference. The data
    members are ordered and contiguous in the MemberList element (packed). Each
    member may be addressed by the dot syntax similar to C such as P.voltage if
    P is the referring parameter and voltage is of a member of P's aggregate
    type. See MemberType, MemberListType, DataEncodingType, NameReferenceType,
    AggregateParameterType and AggregateArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'MemberList'
    member_list: MemberListType = field(
        default_factory=lambda: MemberListType())


@dataclass(slots=True)
class AggregateParameterType:
    """Describe a complex data type analogous to a C-struct. Each field of the
    data type is called a Member. Each Member is part of the MemberList which
    forms the list of items to be placed under this data type's name. The
    MemberList defines a data block and block's size is defined by the
    DataEncodings of each Member's type reference. The data members are ordered
    and contiguous in the MemberList element (packed). Each member may be
    addressed by the dot syntax similar to C such as P.voltage if P is the
    referring parameter and voltage is of a member of P's aggregate type. See
    MemberType, MemberListType, DataEncodingType, NameReferenceType, and
    AggregateDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'MemberList'
    member_list: MemberListType = field(
        default_factory=lambda: MemberListType())


@dataclass(slots=True)
class AlarmConditionsType:
    """Describe up to six levels: Normal, Watch, Warning, Distress, Critical,
    and Severe of conditions the alarm will trigger when true. The types are
    conditions available are a single comparison, a comparison list, a discrete
    lookup list, and custom algorithm. See MatchCriteriaType."""

    watch_alarm: Optional[MatchCriteriaType] = None  # element 'WatchAlarm'
    warning_alarm: Optional[MatchCriteriaType] = None  # element 'WarningAlarm'
    # element 'DistressAlarm'
    distress_alarm: Optional[MatchCriteriaType] = None
    # element 'CriticalAlarm'
    critical_alarm: Optional[MatchCriteriaType] = None
    severe_alarm: Optional[MatchCriteriaType] = None  # element 'SevereAlarm'


@dataclass(slots=True)
class AlarmMultiRangesType:
    """Describe any number of alarm ranges, each with its own level (normal,
    warning, watch, distress, critical, severe) and range form (inside --
    (min,max), [min,max), (min, max], [min, max], or outside -- (-inf, min) or
    (-inf,min] and [max, +inf) or (max,+inf). Ranges may overlap, be disjoint
    and so forth. Ranges within the value spectrum non-specified are non-normal.
    The most severe range level of value within the ranges is the level of the
    alarm. Range values are in calibrated engineering units. See FloatRangeType.
    """

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    range: list[MultiRangeType] = field(default_factory=list)  # element 'Range'


@dataclass(slots=True)
class AlarmRangesType:
    """Describe up to six ranges where either less severe ranges are a subset of
    more severe ranges (outside), or more severe ranges are a subset of less
    severe ranges (inside). In both forms, the undefined least severe range is
    normal. Range values are in calibrated engineering units. See
    FloatRangeType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'rangeForm', schema default 'outside'
    range_form: Optional[RangeFormType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    watch_range: Optional[FloatRangeType] = None  # element 'WatchRange'
    warning_range: Optional[FloatRangeType] = None  # element 'WarningRange'
    distress_range: Optional[FloatRangeType] = None  # element 'DistressRange'
    critical_range: Optional[FloatRangeType] = None  # element 'CriticalRange'
    severe_range: Optional[FloatRangeType] = None  # element 'SevereRange'


@dataclass(slots=True)
class AlarmType:
    """Defines a base schema type used to build up the other data type specific
    alarm types. The definition includes a count to go into alarm (minViolations
    - the counts to go out of alarm is the same), a condition style alarm and a
    custom alarm. See AlarmConditionType, CustomAlgorithmType,
    BinaryAlarmConditionType, BooleanAlarmType, BinaryContextAlarmType,
    EnumerationAlarmType, NumericAlarmType, StringAlarmType, TimeAlarmType,
    TimeAlarmConditionType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[AlarmChoice] = None


@dataclass(slots=True)
class AlgorithmSetType:
    """An unordered collection of algorithms"""

    items: list[AlgorithmSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class AlgorithmTextType:
    """This optional element may be used to enter Pseudo or actual code for the
    algorithm. The language for the algorithm is specified with the language
    attribute"""

    # attribute 'language', schema default 'pseudo'
    language: Optional[str] = None
    value: str = ""  # text content


@dataclass(slots=True)
class AliasSetType:
    """Contains an unordered collection of Alias elements to describe alternate
    names or IDs for this named item."""

    alias: list[AliasType] = field(default_factory=list)  # element 'Alias'


@dataclass(slots=True)
class AliasType:
    """Used to contain an alias (alternate) name or ID for the object. For
    example, a parameter may have a mnemonic, an on-board id, and special IDs
    used by various ground software applications; all of these are alias's. Some
    ground system processing equipment has some severe naming restrictions on
    parameters (e.g., names must less then 12 characters, single case or
    integral id's only); their alias's provide a means of capturing each name in
    a "nameSpace". Note: the name is not reference-able (it cannot be used in a
    name reference substituting for the name of the item of interest). See
    NameDescriptionType."""

    name_space: str = ""  # attribute 'nameSpace'
    alias: str = ""  # attribute 'alias'


@dataclass(slots=True)
class AncillaryDataSetType:
    """Describe an unordered collection of ancillary data. AncillaryData
    elements capture platform/program/implementation specific data about the
    parent element object that is non-standard and would not fit into the
    schema. See AncillaryDataType."""

    # element 'AncillaryData'
    ancillary_data: list[AncillaryDataType] = field(default_factory=list)


@dataclass(slots=True)
class AncillaryDataType:
    """Use for any other data associated with a named item. May be used to
    include administrative data (e.g., version, CM or tags) or potentially any
    MIME type. Data may be included or given as an href."""

    name: str = ""  # attribute 'name'
    # attribute 'mimeType', schema default 'text/plain'
    mime_type: Optional[str] = None
    href: Optional[str] = None  # attribute 'href'
    value: str = ""  # text content


@dataclass(slots=True)
class ArgumentANDedConditionsType:
    """Identical to ANDedConditionsType but supports argument instance
    references."""

    items: list[ArgumentANDedConditionsChoice] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentAbsoluteTimeDataType:
    """A base schema type for describing an absolute time data type. Contains an
    absolute (to a known epoch) time. Use the [ISO 8601] extended format
    CCYY-MM-DDThh:mm:ss where "CC" represents the century, "YY" the year, "MM"
    the month and "DD" the day, preceded by an optional leading "-" sign to
    indicate a negative number. If the sign is omitted, "+" is assumed. The
    letter "T" is the date/time separator and "hh", "mm", "ss" represent hour,
    minute and second respectively. Additional digits can be used to increase
    the precision of fractional seconds if desired i.e. the format ss.ss... with
    any number of digits after the decimal point is supported. See
    AbsoluteTimeParameterType and AbsoluteTimeArgumentType. See
    AbsouteTimeParameterType, AbsoluteTimeArgumentType and BaseTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class ArgumentArgumentRefEntryType:
    """Identical to ArgumentRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    argument_ref: str = ""  # attribute 'argumentRef'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentArrayArgumentRefEntryType:
    """Identical to ArrayParameterRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    argument_ref: str = ""  # attribute 'argumentRef'
    # attribute 'lastEntryForThisArrayInstance', schema default 'false'
    last_entry_for_this_array_instance: Optional[bool] = None
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DimensionList'
    dimension_list: Optional[ArgumentDimensionListType] = None


@dataclass(slots=True)
class ArgumentArrayParameterRefEntryType:
    """Identical to ArrayParameterRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    parameter_ref: str = ""  # attribute 'parameterRef'
    # attribute 'lastEntryForThisArrayInstance', schema default 'false'
    last_entry_for_this_array_instance: Optional[bool] = None
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DimensionList'
    dimension_list: Optional[DimensionListType] = None


@dataclass(slots=True)
class ArgumentAssignmentListType:
    """Argument Assignments specialize a MetaCommand or BlockMetaCommand when
    inheriting from another MetaCommand. General argument values can be
    restricted to specific values to further specialize the MetaCommand. Use it
    to "narrow" a MetaCommand from its base MetaCommand by specifying values of
    arguments for example, a power command may be narrowed to a "power on"
    command by assigning the value of an argument to "on". See
    ArgumentAssignmentType and MetaCommandType."""

    # element 'ArgumentAssignment'
    argument_assignment: list[
        ArgumentAssignmentType] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentAssignmentType:
    """Describe an assignment of an argument with a calibrated/engineering
    value. See ArgumentAssignmentListType."""

    argument_name: str = ""  # attribute 'argumentName'
    argument_value: str = ""  # attribute 'argumentValue'


@dataclass(slots=True)
class ArgumentBaseDataType:
    """Identical to BaseDataType but supports argument instance references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentBaseDataChoice] = None


@dataclass(slots=True)
class ArgumentBaseTimeDataType:
    """Identical to BaseTimeDataType but supports argument instance references.
    """

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class ArgumentBinaryDataEncodingType:
    """Identical to BinaryDataEncodingType but supports argument instance
    references."""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    # element 'SizeInBits'
    size_in_bits: ArgumentIntegerValueType = field(
        default_factory=lambda: ArgumentIntegerValueType())
    # element 'FromBinaryTransformAlgorithm'
    from_binary_transform_algorithm: Optional[ArgumentInputAlgorithmType] = None
    # element 'ToBinaryTransformAlgorithm'
    to_binary_transform_algorithm: Optional[ArgumentInputAlgorithmType] = None


@dataclass(slots=True)
class ArgumentBinaryDataType:
    """Identical to BinaryDataType but supports argument instance references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentBinaryDataChoice] = None


@dataclass(slots=True)
class ArgumentBooleanDataType:
    """Identical to BooleanDataType but supports argument instance references.
    """

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    # attribute 'oneStringValue', schema default 'True'
    one_string_value: Optional[str] = None
    # attribute 'zeroStringValue', schema default 'False'
    zero_string_value: Optional[str] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentBooleanDataChoice] = None


@dataclass(slots=True)
class ArgumentBooleanExpressionType:
    """Identical to BooleanExpressionType but supports argument instance
    references."""

    choice: ArgumentBooleanExpressionChoice = field(
        default_factory=lambda: ArgumentBooleanExpressionChoice())


@dataclass(slots=True)
class ArgumentComparisonCheckType:
    """Identical to ComparisonCheckType but supports argument instance
    references."""

    choice1: ArgumentComparisonCheckChoice1 = field(
        default_factory=lambda: ArgumentComparisonCheckChoice1())
    # element 'ComparisonOperator'
    comparison_operator: ComparisonOperatorsType = ComparisonOperatorsType.EMPTY
    choice2: ArgumentComparisonCheckChoice2 = field(
        default_factory=lambda: ArgumentComparisonCheckChoice2())


@dataclass(slots=True)
class ArgumentComparisonListType:
    """Identical to ComparisonListType but supports argument instance
    references."""

    # element 'Comparison'
    comparison: list[ArgumentComparisonType] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentComparisonType:
    """Identical to ComparisonType but supports argument instance references."""

    # attribute 'comparisonOperator', schema default '=='
    comparison_operator: Optional[ComparisonOperatorsType] = None
    value: str = ""  # attribute 'value'
    choice: ArgumentComparisonChoice = field(
        default_factory=lambda: ArgumentComparisonChoice())


@dataclass(slots=True)
class ArgumentContainerRefEntryType:
    """Identical to ContainerRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    container_ref: str = ""  # attribute 'containerRef'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentContainerSegmentRefEntryType:
    """Identical to ContainerSegmentRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    container_ref: str = ""  # attribute 'containerRef'
    order: Optional[int] = None  # attribute 'order'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentDimensionListType:
    """Identical to DimensionListType but supports argument instance references.
    """

    # element 'Dimension'
    dimension: list[ArgumentDimensionType] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentDimensionType:
    """Identical to DimensionType but supports argument instance references."""

    # element 'StartingIndex'
    starting_index: ArgumentIntegerValueType = field(
        default_factory=lambda: ArgumentIntegerValueType())
    # element 'EndingIndex'
    ending_index: ArgumentIntegerValueType = field(
        default_factory=lambda: ArgumentIntegerValueType())


@dataclass(slots=True)
class ArgumentDiscreteLookupListType:
    """Identical to DiscreteLookupListType but supports argument instance
    references."""

    default_value: int = 0  # attribute 'defaultValue'
    # element 'DiscreteLookup'
    discrete_lookup: list[
        ArgumentDiscreteLookupType] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentDiscreteLookupType:
    """Identical to ArgumentDiscreteLookupType but supports argument instance
    references."""

    value: int = 0  # attribute 'value'
    choice: ArgumentDiscreteLookupChoice = field(
        default_factory=lambda: ArgumentDiscreteLookupChoice())


@dataclass(slots=True)
class ArgumentDynamicValueType:
    """Identical to DynamicValueType but supports argument instance references.
    """

    choice: ArgumentDynamicValueChoice = field(
        default_factory=lambda: ArgumentDynamicValueChoice())
    # element 'LinearAdjustment'
    linear_adjustment: Optional[LinearAdjustmentType] = None


@dataclass(slots=True)
class ArgumentEnumeratedDataType:
    """Identical to EnumeratedDataType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentEnumeratedDataChoice] = None
    # element 'EnumerationList'
    enumeration_list: EnumerationListType = field(
        default_factory=lambda: EnumerationListType())


@dataclass(slots=True)
class ArgumentFixedValueEntryType:
    """Identical to FixedValueEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    binary_value: str = ""  # attribute 'binaryValue'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentFloatDataType:
    """Identical to FloatDataType but supports argument instance references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[float] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[FloatSizeInBitsType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentFloatDataChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'


@dataclass(slots=True)
class ArgumentIndirectParameterRefEntryType:
    """Identical to IndirectParameterRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    alias_name_space: Optional[str] = None  # attribute 'aliasNameSpace'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'ParameterInstance'
    parameter_instance: ParameterInstanceRefType = field(
        default_factory=lambda: ParameterInstanceRefType())


@dataclass(slots=True)
class ArgumentInputAlgorithmType:
    """Identical to InputAlgorithmType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'AlgorithmText'
    algorithm_text: Optional[AlgorithmTextType] = None
    # element 'ExternalAlgorithmSet'
    external_algorithm_set: Optional[ExternalAlgorithmSetType] = None
    input_set: Optional[ArgumentInputSetType] = None  # element 'InputSet'


@dataclass(slots=True)
class ArgumentInputSetType:
    """Identical to InputSetType but supports argument instance references."""

    items: list[ArgumentInputSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentInstanceRefType:
    """An argument instance is the name of an argument as the reference is
    always resolved locally to the metacommand."""

    argument_ref: str = ""  # attribute 'argumentRef'
    # attribute 'useCalibratedValue', schema default 'true'
    use_calibrated_value: Optional[bool] = None


@dataclass(slots=True)
class ArgumentIntegerDataType:
    """Identical to IntegerDataType but supports argument instance references.
    """

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[int] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[int] = None
    signed: Optional[bool] = None  # attribute 'signed', schema default 'true'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentIntegerDataChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'


@dataclass(slots=True)
class ArgumentIntegerValueType:
    """Identical to IntegerValueType but supports argument instance references.
    """

    choice: ArgumentIntegerValueChoice = field(
        default_factory=lambda: ArgumentIntegerValueChoice())


@dataclass(slots=True)
class ArgumentListType:
    """Defines a list of Arguments for a command definition."""

    # element 'Argument'
    argument: list[ArgumentType] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentLocationInContainerInBitsType:
    """Identical to LocationInContainerInBitsType but supports argument instance
    references."""

    # attribute 'referenceLocation', schema default 'previousEntry'
    reference_location: Optional[ReferenceLocationType] = None
    choice: ArgumentLocationInContainerInBitsChoice = field(
        default_factory=lambda: ArgumentLocationInContainerInBitsChoice())


@dataclass(slots=True)
class ArgumentMatchCriteriaType:
    """Identical to MatchCriteriaType but supports argument instance references.
    """

    choice: ArgumentMatchCriteriaChoice = field(
        default_factory=lambda: ArgumentMatchCriteriaChoice())


@dataclass(slots=True)
class ArgumentMathOperationType:
    """Describe a value to set to a destination Parameter after completion of a
    commanding lifecycle step."""

    items: list[ArgumentMathOperationChoice] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentORedConditionsType:
    """Identical to ORedConditionsType but supports argument instance
    references."""

    items: list[ArgumentORedConditionsChoice] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentParameterRefEntryType:
    """Identical to ParameterRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    parameter_ref: str = ""  # attribute 'parameterRef'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentParameterSegmentRefEntryType:
    """Identical to ParameterSegmentRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    parameter_ref: str = ""  # attribute 'parameterRef'
    order: Optional[int] = None  # attribute 'order'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentRelativeTimeDataType:
    """Used to contain a relative time value. Used to describe a relative time.
    Normally used for time offsets. A Relative time is expressed as PnYn MnDTnH
    nMnS, where nY represents the number of years, nM the number of months, nD
    the number of days, 'T' is the date/time separator, nH the number of hours,
    nM the number of minutes and nS the number of seconds. The number of seconds
    can include decimal digits to arbitrary precision. For example, to indicate
    a duration of 1 year, 2 months, 3 days, 10 hours, and 30 minutes, one would
    write: P1Y2M3DT10H30M. One could also indicate a duration of minus 120 days
    as: -P120D. An extension of Schema duration type."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class ArgumentRepeatType:
    """Identical to RepeatType but supports argument instance references."""

    # element 'Count'
    count: ArgumentIntegerValueType = field(
        default_factory=lambda: ArgumentIntegerValueType())
    offset: Optional[ArgumentIntegerValueType] = None  # element 'Offset'


@dataclass(slots=True)
class ArgumentRestrictionListType:
    """Defines a list of argument values that restrict a constraint from being
    realized in the commanding lifecycle."""

    # element 'ArgumentRestriction'
    argument_restriction: list[
        ArgumentAssignmentType] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentSequenceEntryType:
    """Identical to a SequenceEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentStreamSegmentEntryType:
    """Identical to StreamRefEntryType but supports argument instance
    references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    stream_ref: str = ""  # attribute 'streamRef'
    order: Optional[int] = None  # attribute 'order'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        ArgumentLocationInContainerInBitsType] = None
    repeat_entry: Optional[ArgumentRepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[ArgumentMatchCriteriaType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentStringDataEncodingType:
    """Identical to StringDataEncodingType but supports argument instance
    references."""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # attribute 'encoding', schema default 'UTF-8'
    encoding: Optional[StringEncodingType] = None
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    choice: ArgumentStringDataEncodingChoice = field(
        default_factory=lambda: ArgumentStringDataEncodingChoice())


@dataclass(slots=True)
class ArgumentStringDataType:
    """Identical to StringDataType but supports argument instance references."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    restriction_pattern: Optional[str] = None  # attribute 'restrictionPattern'
    # attribute 'characterWidth'
    character_width: Optional[CharacterWidthType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[ArgumentStringDataChoice] = None
    # element 'SizeRangeInCharacters'
    size_range_in_characters: Optional[IntegerRangeType] = None


@dataclass(slots=True)
class ArgumentType:
    """An Argument has a name and can take on values with the underlying value
    type described by the ArgumentTypeRef. Describe the properties of a command
    argument referring to a data type (argument type). The bulk of properties
    associated with a command argument are in its argument type. The initial
    value specified here, overrides the initial value in the argument type. See
    BaseDataType, BaseTimeDataType and NameReferenceType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    argument_type_ref: str = ""  # attribute 'argumentTypeRef'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArgumentTypeSetType:
    """Describe an unordered collection of argument type definitions. These
    types named for the engineering/calibrated type of the argument. See
    BaseDataType and BaseTimeDataType."""

    items: list[ArgumentTypeSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class ArgumentVariableStringType:
    """Identical to VariableStringType but supports argument instance
    references."""

    max_size_in_bits: int = 0  # attribute 'maxSizeInBits'
    choice1: Optional[ArgumentVariableStringChoice1] = None
    choice2: ArgumentVariableStringChoice2 = field(
        default_factory=lambda: ArgumentVariableStringChoice2())


@dataclass(slots=True)
class ArrayArgumentType:
    """Describe an array argument type. The size and number of dimension are
    described here. See ArrayParameterRefEntryType, NameReferenceType and
    ArrayDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    array_type_ref: str = ""  # attribute 'arrayTypeRef'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DimensionList'
    dimension_list: ArgumentDimensionListType = field(
        default_factory=lambda: ArgumentDimensionListType())


@dataclass(slots=True)
class ArrayDataTypeType:
    """A base schema type for describing an array data type. The number of and
    size of each dimension is defined in its two child types. See
    NameReferenceType, ArrayArgumentType and ArrayParameterType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    array_type_ref: str = ""  # attribute 'arrayTypeRef'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ArrayParameterRefEntryType:
    """Describe an entry that is an array parameter. Specify the dimension sizes
    if you subsetting the array (the number of dimensions shall match the number
    defined in the parameter's type definition), otherwise the ones in the
    ParameterType are assumed. See SequenceEntryType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    parameter_ref: str = ""  # attribute 'parameterRef'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DimensionList'
    dimension_list: Optional[DimensionListType] = None


@dataclass(slots=True)
class ArrayParameterType:
    """Describe an array parameter type. The size and number of dimensions are
    described here. See ArrayParameterRefEntryType, NameReferenceType and
    ArrayDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    array_type_ref: str = ""  # attribute 'arrayTypeRef'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DimensionList'
    dimension_list: DimensionListType = field(
        default_factory=lambda: DimensionListType())


@dataclass(slots=True)
class AuthorSetType:
    """Describe an unordered collection of authors. See AuthorType."""

    author: list[str] = field(default_factory=list)  # element 'Author'


@dataclass(slots=True)
class AutoInvertType:
    """After searching for the frame sync marker for some number of bits, it may
    be desirable to invert the incoming data, and then look for frame sync. In
    some cases this will require an external algorithm"""

    # attribute 'badFramesToAutoInvert', schema default '1024'
    bad_frames_to_auto_invert: Optional[int] = None
    # element 'InvertAlgorithm'
    invert_algorithm: Optional[InputAlgorithmType] = None


@dataclass(slots=True)
class BaseAlarmType:
    """Supplies an optional non-reference-able name and short description for
    alarms. Also includes an optional ancillary data for any special local
    flags, note that these may not necessarily transfer to another recipient of
    an instance document."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class BaseCalibratorType:
    """Supplies an optional non-reference-able name and short description for
    calibrators. Also includes an optional ancillary data for any special local
    flags, note that these may not necessarily transfer to another recipient of
    an instance document."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class BaseComparisonType:
    """A base type for comparison related elements that improves the mapping
    produced by data binding tools."""


@dataclass(slots=True)
class BaseConditionsType:
    """A base type for boolean expression related elements that improves the
    mapping produced by data binding tools."""


@dataclass(slots=True)
class BaseContainerType:
    """Describe a child/parent container inheritance relationship. Describe
    constraints with RestrictionCriteria, conditions that must be true for this
    container to be an extension of the parent container. A constraint can be
    used to convey the identifying features of the telemetry format such as the
    CCSDS application id or minor-frame id. See RestrictionCriteriaType and
    SequenceContainerType."""

    container_ref: str = ""  # attribute 'containerRef'
    # element 'RestrictionCriteria'
    restriction_criteria: Optional[RestrictionCriteriaType] = None


@dataclass(slots=True)
class BaseDataType:
    """An abstract schema type used by within the schema to derive the other
    simple/primitive engineering form data types: BooleanDataType,
    BinaryDataType, StringDataType, EnumeratedDataType, FloatDataType and
    IntegerDataType. The encoding elements are optional because they describe
    the raw wire encoded form of the data type. Encoding is only necessary when
    the type is telemetered in some form. Local variables and derived typically
    do not require encoding."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BaseDataChoice] = None


@dataclass(slots=True)
class BaseMetaCommandType:
    """When specified, a BaseMetaCommand element identifies that this
    MetaCommand inherits (extends) another MetaCommand. It's required
    ArgumentAssignmentList narrows or this command from the parent. This is
    typically used when specializing a generic MetaCommand to a specific
    MetaCommand. See MetaCommandType."""

    meta_command_ref: str = ""  # attribute 'metaCommandRef'
    # element 'ArgumentAssignmentList'
    argument_assignment_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class BaseTimeDataType:
    """An abstract schema type used within the schema to derive other time based
    data types: RelativeTimeDataType and AbsoluteTimeDataType. An absolute time
    data type is a telemetered source/destination data type. A data encoding
    must be set. An optional epoch may be set. Time types are an exception to
    other primitives because, if the time data type is not telemetered, it still
    must have a data encoding set. See DataEncodingType, AbsoluteTimeDataType
    and RelativeTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class BaseTriggerType:
    """A base type for the various triggers, purely to improve the mappings
    created by data binding compilers."""


@dataclass(slots=True)
class BinaryAlarmType:
    """Describe alarm conditions specific to the binary data type, extends the
    basic AlarmType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[BinaryAlarmChoice] = None


@dataclass(slots=True)
class BinaryArgumentType:
    """Defines a binary engineering/calibrated argument type (often called "blob
    type"). The binary data may be of fixed or variable length, and has an
    optional encoding and decoding algorithm that may be defined to transform
    the data between space and ground. See BinaryDataEncodingType,
    IntegerValueType, InputAlgorithmType, and BinaryDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BinaryArgumentChoice] = None


@dataclass(slots=True)
class BinaryContextAlarmListType:
    """Describe an ordered collection of context binary alarms, duplicates are
    valid. Process the contexts in list order. See BinaryContextAlarmType."""

    # element 'ContextAlarm'
    context_alarm: list[BinaryContextAlarmType] = field(default_factory=list)


@dataclass(slots=True)
class BinaryContextAlarmType:
    """XTCE type."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[BinaryContextAlarmChoice] = None
    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())


@dataclass(slots=True)
class BinaryDataEncodingType:
    """Describe binary data that is unmolested in the decoding/encoding or
    cannot be represented in any of the other data encoding formats. Optionally
    use the FromBinaryTransformAlgorithm and ToBinaryTransformAlgorithm element
    to describe the transformation process. See InputAlgorithmType for the
    transformation structure."""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    # element 'SizeInBits'
    size_in_bits: IntegerValueType = field(
        default_factory=lambda: IntegerValueType())
    # element 'FromBinaryTransformAlgorithm'
    from_binary_transform_algorithm: Optional[InputAlgorithmType] = None
    # element 'ToBinaryTransformAlgorithm'
    to_binary_transform_algorithm: Optional[InputAlgorithmType] = None


@dataclass(slots=True)
class BinaryDataType:
    """A base schema type for describing a binary data engineering/calibrated
    type (often called "blob type"). The binary data may be of fixed or variable
    length, and has an optional encoding and decoding algorithm that may be
    defined to transform the data between space and ground. See BaseDataType,
    BinaryParameterType and BinaryArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BinaryDataChoice] = None


@dataclass(slots=True)
class BinaryParameterType:
    """Describe a binary engineering/calibrated parameter type (sometimes called
    a "blob type"). It may be of fixed or variable length, and has an optional
    encoding and decoding algorithm that may be defined to transform the data
    between space and ground. See BinaryDataEncodingType, IntegerValueType,
    InputAlgorithmType and BinaryDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BinaryParameterChoice] = None
    default_alarm: Optional[BinaryAlarmType] = None  # element 'DefaultAlarm'
    # element 'ContextAlarmList'
    context_alarm_list: Optional[BinaryContextAlarmListType] = None


@dataclass(slots=True)
class BlockMetaCommandType:
    """Describe an ordered grouping of MetaCommands into a list, duplicates are
    valid. The block contains argument values fully specified. See
    MetaCommandStepListType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'MetaCommandStepList'
    meta_command_step_list: MetaCommandStepListType = field(
        default_factory=lambda: MetaCommandStepListType())


@dataclass(slots=True)
class BooleanAlarmType:
    """Alarm conditions for Boolean types"""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[BooleanAlarmChoice] = None


@dataclass(slots=True)
class BooleanArgumentType:
    """Defines a boolean argument type which has two values only: "True" (1) or
    "False" (0). The values one and zero may be mapped to a specific string
    using the attributes oneStringValue and zeroStringValue. This type is a
    simplified form of the EnumeratedDataType. See IntegerDataEncoding and
    BooleanDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    # attribute 'oneStringValue', schema default 'True'
    one_string_value: Optional[str] = None
    # attribute 'zeroStringValue', schema default 'False'
    zero_string_value: Optional[str] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BooleanArgumentChoice] = None


@dataclass(slots=True)
class BooleanContextAlarmListType:
    """Describe an ordered collection of context boolean alarms, duplicates are
    valid. Process the contexts in list order. See BooleanContextAlarmType."""

    # element 'ContextAlarm'
    context_alarm: list[BooleanContextAlarmType] = field(default_factory=list)


@dataclass(slots=True)
class BooleanContextAlarmType:
    """XTCE type."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[BooleanContextAlarmChoice] = None
    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())


@dataclass(slots=True)
class BooleanDataType:
    """A base schema type for describing a boolean data type which has two
    values only: "True" (1) or "False" (0). The values one and zero may be
    mapped to a specific string using the attributes oneStringValue and
    zeroStringValue. This type is a simplified form of the EnumeratedDataType.
    See BaseDataType, BooleanParameterType and BooleanArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    # attribute 'oneStringValue', schema default 'True'
    one_string_value: Optional[str] = None
    # attribute 'zeroStringValue', schema default 'False'
    zero_string_value: Optional[str] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BooleanDataChoice] = None


@dataclass(slots=True)
class BooleanExpressionType:
    """Holds an arbitrarily complex boolean expression"""

    choice: BooleanExpressionChoice = field(
        default_factory=lambda: BooleanExpressionChoice())


@dataclass(slots=True)
class BooleanParameterType:
    """Describe a boolean parameter type which has two values only: "True" (1)
    or "False" (0). The values one and zero may be mapped to a specific string
    using the attributes oneStringValue and zeroStringValue. This type is a
    simplified form of the EnumeratedDataType. See IntegerDataEncoding and
    BooleanDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    # attribute 'oneStringValue', schema default 'True'
    one_string_value: Optional[str] = None
    # attribute 'zeroStringValue', schema default 'False'
    zero_string_value: Optional[str] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[BooleanParameterChoice] = None
    default_alarm: Optional[BooleanAlarmType] = None  # element 'DefaultAlarm'
    # element 'ContextAlarmList'
    context_alarm_list: Optional[BooleanContextAlarmListType] = None


@dataclass(slots=True)
class ByteType:
    """XTCE type."""

    byte_significance: int = 0  # attribute 'byteSignificance'


@dataclass(slots=True)
class CRCType:
    """Cyclic Redundancy Check (CRC) definition. The polynomial coefficients for
    the CRC are defined as a truncated hex value. The coefficient for the nth
    bit of an n-bit CRC will always be 1 and is not represented in the truncated
    hex value. For example, the truncated hex value of CRC-32 (width=32 bits)
    used in the Ethernet specification is 0x04C11DB7, where each non-zero bit of
    the truncated hex represents a coefficient of 1 in the polynomial and the
    bit position represents the exponent. There may also be an initial remainder
    "InitRemainder" and a final XOR "FinalXOR" to fully specify the CRC.
    reflectData and reflectRemainder may also be specified to reverse the bit
    order in the incoming data and/or the result."""

    width: int = 0  # attribute 'width'
    # attribute 'reflectData', schema default 'false'
    reflect_data: Optional[bool] = None
    # attribute 'reflectRemainder', schema default 'false'
    reflect_remainder: Optional[bool] = None
    # attribute 'direction', schema default 'mostSignificantBitFirst'
    direction: Optional[BitOrderType] = None
    # attribute 'bitsFromReference', schema default '0'
    bits_from_reference: Optional[int] = None
    # attribute 'reference', schema default 'start'
    reference: Optional[ReferencePointType] = None
    parameter_ref: Optional[str] = None  # attribute 'parameterRef'
    polynomial: str = ""  # element 'Polynomial'
    init_remainder: Optional[str] = None  # element 'InitRemainder'
    final_xor: Optional[str] = None  # element 'FinalXOR'


@dataclass(slots=True)
class CalibratorType:
    """Describe a calibrator to transform a source data type raw/uncalibrated
    value (e.g. an integer count from a spacecraft) to an engineering
    unit/calibrated value for users (e.g. a float)."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: CalibratorChoice = field(default_factory=lambda: CalibratorChoice())


@dataclass(slots=True)
class ChangeAlarmRangesType:
    """Describe an alarm when the parameter value's rate-of-change is either too
    fast or too slow. The change may be with respect to time (the default) or
    with respect to samples (delta alarms). Use the changeType attribute to
    select the type: changePerSecond (time) or changePerSample (delta). The
    change may also be ether relative (as a percentage change) or absolute as
    set by the changeBasis attribute. (Delta alarms are typically absolute but
    percentage is conceivable). The alarm also requires the spanOfInterest in
    both samples and seconds to have passed before it is to trigger. For time
    based rate of change alarms, the time specified in spanOfInterestInSeconds
    is used to calculate the change. For sample based rate of change alarms, the
    change is calculated over the number of samples specified in
    spanOfInterestInSamples. A typical delta alarm would set:
    changeType=changePerSample, changeBasis=absoluteChange,
    spanOfInterestInSamples=1. A typical time based version would set:
    changeType=changePerSecond, changeBasis=percentageChange, and
    spaceOfInterestInSeconds=1. To set the ranges use maxInclusive, the
    following definition applies: | Normal.maxInclusive | <= |
    Watch.maxInclusive | <= | Warning.maxInclusive | <= | Distress.maxInclusive
    | <= | Critical.maxInclusive | <= | Severe.maxInclusive |. And it is further
    assumed the absolute value of each range and sampled value it taken to
    evaluate the alarm. See NumericAlarmType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'rangeForm', schema default 'outside'
    range_form: Optional[RangeFormType] = None
    # attribute 'changeType', schema default 'changePerSecond'
    change_type: Optional[ChangeSpanType] = None
    # attribute 'changeBasis', schema default 'absoluteChange'
    change_basis: Optional[ChangeBasisType] = None
    # attribute 'spanOfInterestInSamples', schema default '1'
    span_of_interest_in_samples: Optional[int] = None
    # attribute 'spanOfInterestInSeconds', schema default '0'
    span_of_interest_in_seconds: Optional[float] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    watch_range: Optional[FloatRangeType] = None  # element 'WatchRange'
    warning_range: Optional[FloatRangeType] = None  # element 'WarningRange'
    distress_range: Optional[FloatRangeType] = None  # element 'DistressRange'
    critical_range: Optional[FloatRangeType] = None  # element 'CriticalRange'
    severe_range: Optional[FloatRangeType] = None  # element 'SevereRange'


@dataclass(slots=True)
class ChangeValueType:
    """Describe a change value used to test verification status. See
    CommandVerifierType."""

    value: float = 0.0  # attribute 'value'


@dataclass(slots=True)
class CheckWindowAlgorithmsType:
    """Used by CommandVerifiers to limit the time allocated to check for the
    verification. See CommandVerifierType."""

    # element 'StartCheck'
    start_check: InputAlgorithmType = field(
        default_factory=lambda: InputAlgorithmType())
    # element 'StopTime'
    stop_time: InputAlgorithmType = field(
        default_factory=lambda: InputAlgorithmType())


@dataclass(slots=True)
class CheckWindowType:
    """Used by CommandVerifiers to limit the time allocated to check for the
    verification. See CheckWindowAlgorithmsType."""

    # attribute 'timeToStartChecking'
    time_to_start_checking: Optional[str] = None
    time_to_stop_checking: str = ""  # attribute 'timeToStopChecking'
    # attribute 'timeWindowIsRelativeTo', schema default
    # 'timeLastVerifierPassed'
    time_window_is_relative_to: Optional[TimeWindowIsRelativeToType] = None


@dataclass(slots=True)
class ChecksumType:
    """Describe checksum or hash function definiton. See ErrorDetectCorrectType.
    """

    # attribute 'bitsFromReference', schema default '0'
    bits_from_reference: Optional[int] = None
    # attribute 'reference', schema default 'start'
    reference: Optional[ReferencePointType] = None
    name: ChecksumnameType = ChecksumnameType.UNIX_SUM  # attribute 'name'
    hash_size_in_bits: Optional[int] = None  # attribute 'hashSizeInBits'
    parameter_ref: Optional[str] = None  # attribute 'parameterRef'
    # element 'InputAlgorithm'
    input_algorithm: Optional[InputAlgorithmType] = None


@dataclass(slots=True)
class CommandContainerEntryListType:
    """Describe an entry list for a CommandContainer which is associated with a
    MetaCommand. The entry list for a MetaCommand CommandContainer element
    operates in a similar fashion as the entry list element for a
    SequenceContainer element. It adds fixed value and argument entries to the
    entry list not present in sequence containers. See MetaCommandType,
    CommandContainerType and EntryListType."""

    items: list[CommandContainerEntryListChoice] = field(default_factory=list)


@dataclass(slots=True)
class CommandContainerSetType:
    """Contains an unordered Set of Command Containers"""

    # element 'CommandContainer'
    command_container: list[SequenceContainerType] = field(default_factory=list)


@dataclass(slots=True)
class CommandContainerType:
    """Describe a MetaCommand command container. The command container may
    contain arguments, parameters, other basic containers, and fixed values.
    Arguments are supplied by the user of a commanding application; parameters
    are supplied by the controlling system. Parameters and arguments map source
    data types to encodings. See MetaCommandType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DefaultRateInStream'
    default_rate_in_stream: Optional[RateInStreamType] = None
    # element 'RateInStreamSet'
    rate_in_stream_set: Optional[RateInStreamSetType] = None
    # element 'BinaryEncoding'
    binary_encoding: Optional[ContainerBinaryDataEncodingType] = None
    # element 'EntryList'
    entry_list: CommandContainerEntryListType = field(
        default_factory=lambda: CommandContainerEntryListType())
    # element 'BaseContainer'
    base_container: Optional[BaseContainerType] = None


@dataclass(slots=True)
class CommandMetaDataType:
    """Describe command related metadata. Items defined in this area may refer
    to items defined in TelemetryMetaData. See TelemetryMetaDataType."""

    # element 'ParameterTypeSet'
    parameter_type_set: Optional[ParameterTypeSetType] = None
    parameter_set: Optional[ParameterSetType] = None  # element 'ParameterSet'
    # element 'ArgumentTypeSet'
    argument_type_set: Optional[ArgumentTypeSetType] = None
    # element 'MetaCommandSet'
    meta_command_set: Optional[MetaCommandSetType] = None
    # element 'CommandContainerSet'
    command_container_set: Optional[CommandContainerSetType] = None
    stream_set: Optional[StreamSetType] = None  # element 'StreamSet'
    algorithm_set: Optional[AlgorithmSetType] = None  # element 'AlgorithmSet'


@dataclass(slots=True)
class CommandVerifierType:
    """A command verifier is used to check that the command has been
    successfully executed. Command Verifiers may be either a Custom Algorithm or
    a Boolean Check or the presence of a Container for a relative change in the
    value of a Parameter. The CheckWindow is a time period where the
    verification must test true to pass."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: CommandVerifierChoice1 = field(
        default_factory=lambda: CommandVerifierChoice1())
    choice2: CommandVerifierChoice2 = field(
        default_factory=lambda: CommandVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class ComparisonCheckType:
    """Describe the comparison between the instance (value) of a parameter
    against either a specified value or another parameter instance."""

    # element 'ParameterInstanceRef'
    parameter_instance_ref: ParameterInstanceRefType = field(
        default_factory=lambda: ParameterInstanceRefType())
    # element 'ComparisonOperator'
    comparison_operator: ComparisonOperatorsType = ComparisonOperatorsType.EMPTY
    choice: ComparisonCheckChoice = field(
        default_factory=lambda: ComparisonCheckChoice())


@dataclass(slots=True)
class ComparisonListType:
    """All comparisons must be true"""

    # element 'Comparison'
    comparison: list[ComparisonType] = field(default_factory=list)


@dataclass(slots=True)
class ComparisonType:
    """A simple ParameterInstanceRef to value comparison. The string supplied in
    the value attribute needs to be converted to a type matching the Parameter
    being compared to. For integer types it is base 10 form. Floating point
    types may be specified in normal (100.0) or scientific (1.0e2) form. The
    value is truncated to use the least significant bits that match the bit size
    of the Parameter being compared to."""

    parameter_ref: str = ""  # attribute 'parameterRef'
    instance: Optional[int] = None  # attribute 'instance', schema default '0'
    # attribute 'useCalibratedValue', schema default 'true'
    use_calibrated_value: Optional[bool] = None
    # attribute 'comparisonOperator', schema default '=='
    comparison_operator: Optional[ComparisonOperatorsType] = None
    value: str = ""  # attribute 'value'


@dataclass(slots=True)
class CompleteVerifierType:
    """A possible set of verifiers that all must be true for the command be
    considered completed. Consider that some may not participate due to argument
    value restriction, if that element is used."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: CompleteVerifierChoice1 = field(
        default_factory=lambda: CompleteVerifierChoice1())
    choice2: CompleteVerifierChoice2 = field(
        default_factory=lambda: CompleteVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None
    # element 'ReturnParmRef'
    return_parm_ref: Optional[ParameterRefType] = None


@dataclass(slots=True)
class ConstantType:
    """Names and provides a value for a constant input to the algorithm. There
    are two attributes to Constant, constantName and value. constantName is a
    variable name in the algorithm to be executed. value is the value of the
    constant to be used."""

    constant_name: str = ""  # attribute 'constantName'
    value: str = ""  # attribute 'value'


@dataclass(slots=True)
class ContainerBinaryDataEncodingType:
    """Describe container binary data that is unmolested in the
    decoding/encoding or cannot be represented in any of the other data encoding
    formats. Optionally use the FromBinaryTransformAlgorithm and
    ToBinaryTransformAlgorithm element to describe the transformation process.
    See InputAlgorithmType for the transformation structure."""

    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    size_in_bits: Optional[IntegerValueType] = None  # element 'SizeInBits'
    # element 'FromBinaryTransformAlgorithm'
    from_binary_transform_algorithm: Optional[InputAlgorithmType] = None
    # element 'ToBinaryTransformAlgorithm'
    to_binary_transform_algorithm: Optional[InputAlgorithmType] = None


@dataclass(slots=True)
class ContainerRefEntryType:
    """An entry that is simply a reference to another container."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    container_ref: str = ""  # attribute 'containerRef'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ContainerRefSetType:
    """XTCE type."""

    # element 'ContainerRef'
    container_ref: list[ContainerRefType] = field(default_factory=list)


@dataclass(slots=True)
class ContainerRefType:
    """Holds a reference to a container"""

    container_ref: str = ""  # attribute 'containerRef'


@dataclass(slots=True)
class ContainerSegmentRefEntryType:
    """An entry that is only a portion of a container indicating that the entire
    container must be assembled from other container segments. It is assumed
    that container segments happen sequentially in time, that is the first part
    of a container is first, however (and there's always a however), if this is
    not the case the order of this container segment may be supplied with the
    order attribute where the first segment order="0". Each instance of a
    container cannot overlap in the overall sequence with another instance"""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    container_ref: str = ""  # attribute 'containerRef'
    order: Optional[int] = None  # attribute 'order'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ContainerSetType:
    """Unordered Set of Containers"""

    # element 'SequenceContainer'
    sequence_container: list[
        SequenceContainerType] = field(default_factory=list)


@dataclass(slots=True)
class ContainerType:
    """An abstract block of data; used as the base type for more specific
    container types"""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DefaultRateInStream'
    default_rate_in_stream: Optional[RateInStreamType] = None
    # element 'RateInStreamSet'
    rate_in_stream_set: Optional[RateInStreamSetType] = None
    # element 'BinaryEncoding'
    binary_encoding: Optional[ContainerBinaryDataEncodingType] = None


@dataclass(slots=True)
class ContextCalibratorListType:
    """Describe an ordered list of calibrators with a context match. Useful when
    different calibrations must be used depending on a matching value. The first
    context that matches determines which calibrator to use. See
    IntegerDataEncodingType and FloatDataEncodingType."""

    # element 'ContextCalibrator'
    context_calibrator: list[
        ContextCalibratorType] = field(default_factory=list)


@dataclass(slots=True)
class ContextCalibratorType:
    """Context calibrations are applied when the ContextMatch is true. Context
    calibrators overide Default calibrators"""

    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())
    # element 'Calibrator'
    calibrator: CalibratorType = field(default_factory=lambda: CalibratorType())


@dataclass(slots=True)
class ContextMatchType:
    """A MatchCriteriaType used for Context selection. It is possible that no
    match evaluates to true, which results in the default element being used. It
    is also possible that a match can have an empty context change, in which
    case the default is replaced with nothing."""

    choice: ContextMatchChoice = field(
        default_factory=lambda: ContextMatchChoice())


@dataclass(slots=True)
class ContextSignificanceListType:
    """Describe an ordered list of ContextSignificance elements where the
    significance on the first context match to test true is used as the
    significance of the MetaCommand. If there is a DefaultSignificance, it is
    overrideen by the matching context. See ContextSignificantType and
    MetaCommandType."""

    # element 'ContextSignificance'
    context_significance: list[
        ContextSignificanceType] = field(default_factory=list)


@dataclass(slots=True)
class ContextSignificanceType:
    """Describe a significance level for a MetaCommand definition where the
    significance level depends on matching a context value. See ContextMatchType
    and SignificanceType."""

    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())
    # element 'Significance'
    significance: SignificanceType = field(
        default_factory=lambda: SignificanceType())


@dataclass(slots=True)
class CustomAlarmType:
    """Describe a custom, algorithmic alarm condition. The algorithm is assumed
    to return a boolean value: true or false. See AlarmType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'InputAlgorithm'
    input_algorithm: InputAlgorithmType = field(
        default_factory=lambda: InputAlgorithmType())


@dataclass(slots=True)
class CustomStreamType:
    """A stream type where some level of custom processing (e.g. convolutional,
    encryption, compression) is performed. Has a reference to external
    algorithms for encoding and decoding algorithms."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    bit_rate_in_bps: Optional[float] = None  # attribute 'bitRateInBPS'
    # attribute 'pcmType', schema default 'NRZL'
    pcm_type: Optional[PCMType] = None
    # attribute 'inverted', schema default 'false'
    inverted: Optional[bool] = None
    encoded_stream_ref: str = ""  # attribute 'encodedStreamRef'
    decoded_stream_ref: str = ""  # attribute 'decodedStreamRef'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'EncodingAlgorithm'
    encoding_algorithm: InputAlgorithmType = field(
        default_factory=lambda: InputAlgorithmType())
    # element 'DecodingAlgorithm'
    decoding_algorithm: InputOutputAlgorithmType = field(
        default_factory=lambda: InputOutputAlgorithmType())


@dataclass(slots=True)
class DataEncodingType:
    """Describes how a particular piece of data is sent or received from some
    non-native, off-platform device. (e.g. a spacecraft)"""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None


@dataclass(slots=True)
class DescriptionType:
    """Defines an abstract schema type used as basis for NameDescriptionType and
    OptionalNameDescriptionType, includes an attribute for a short description
    and an element for a longer unbounded description. This type also provides
    alias set and ancillary data set See AliasSetType and AncillaryDataSetType.
    """

    short_description: Optional[str] = None  # attribute 'shortDescription'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class DimensionListType:
    """Where the Dimension list is in this form: Array[1stDim][2ndDim][lastDim].
    The last dimension is assumed to be the least significant - that is this
    dimension will cycle through its combination before the next to last
    dimension changes. The order MUST ascend or the array will need to be broken
    out entry by entry."""

    # element 'Dimension'
    dimension: list[DimensionType] = field(default_factory=list)


@dataclass(slots=True)
class DimensionType:
    """For partial entries of an array, the starting and ending index for each
    dimension, OR the Size must be specified. Indexes are zero based."""

    # element 'StartingIndex'
    starting_index: IntegerValueType = field(
        default_factory=lambda: IntegerValueType())
    # element 'EndingIndex'
    ending_index: IntegerValueType = field(
        default_factory=lambda: IntegerValueType())


@dataclass(slots=True)
class DiscreteLookupListType:
    """Describe an ordered table of integer values and associated conditions,
    forming a lookup table. The list may have duplicates. The table is evaluated
    from first to last, the first condition to be true returns the value
    associated with it. See DiscreteLookupType."""

    default_value: int = 0  # attribute 'defaultValue'
    # element 'DiscreteLookup'
    discrete_lookup: list[DiscreteLookupType] = field(default_factory=list)


@dataclass(slots=True)
class DiscreteLookupType:
    """Describe a discrete value lookup and the value associated when the lookup
    evaluates to true."""

    value: int = 0  # attribute 'value'
    choice: DiscreteLookupChoice = field(
        default_factory=lambda: DiscreteLookupChoice())


@dataclass(slots=True)
class DynamicValueType:
    """Uses a parameter instance to obtain the value. The parameter value may be
    optionally adjusted by a Linear function or use a series of boolean
    expressions to lookup the value. Anything more complex and a DynamicValue
    with a CustomAlgorithm may be used"""

    # element 'ParameterInstanceRef'
    parameter_instance_ref: ParameterInstanceRefType = field(
        default_factory=lambda: ParameterInstanceRefType())
    # element 'LinearAdjustment'
    linear_adjustment: Optional[LinearAdjustmentType] = None


@dataclass(slots=True)
class EncodingType:
    """Describe the data encoding for a time data type. It includes the units
    and other attributes scale and offset. Use scale and offset to describe a
    y=mx+b relationship (where m is the slope/scale and b is the
    intercept/offset) to make adjustments to the encoded time value so that it
    matches the time units. For binary encoded time use transform algorithms to
    convert time data formats that are too difficult to describe in XTCE. See
    AbsoluteTimeDataType and RelativeTimeDataType."""

    # attribute 'units', schema default 'seconds'
    units: Optional[TimeUnitsType] = None
    scale: Optional[float] = None  # attribute 'scale', schema default '1'
    offset: Optional[float] = None  # attribute 'offset', schema default '0'
    choice: EncodingChoice = field(default_factory=lambda: EncodingChoice())


@dataclass(slots=True)
class EntryListType:
    """Contains an ordered list of Entries. Used in Sequence Container"""

    items: list[EntryListChoice] = field(default_factory=list)


@dataclass(slots=True)
class EnumeratedArgumentType:
    """Describes an enumerated argument type. The enumeration list consists of
    label/value pairs. See EnumerationListType, IntegerDataEncodingType and
    EnumeratedDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[EnumeratedArgumentChoice] = None
    # element 'EnumerationList'
    enumeration_list: EnumerationListType = field(
        default_factory=lambda: EnumerationListType())


@dataclass(slots=True)
class EnumeratedDataType:
    """Describes an enumerated parameter type. The enumeration list consists of
    label/value pairs. See EnumerationListType, EnumeratedParameterType and
    EnumeratedArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[EnumeratedDataChoice] = None
    # element 'EnumerationList'
    enumeration_list: EnumerationListType = field(
        default_factory=lambda: EnumerationListType())


@dataclass(slots=True)
class EnumeratedParameterType:
    """Describe an enumerated parameter type. The enumeration list consists of
    label/value pairs. See EnumerationListType, IntegerDataEncodingType and
    EnumeratedDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[EnumeratedParameterChoice] = None
    # element 'EnumerationList'
    enumeration_list: EnumerationListType = field(
        default_factory=lambda: EnumerationListType())
    # element 'DefaultAlarm'
    default_alarm: Optional[EnumerationAlarmType] = None
    # element 'ContextAlarmList'
    context_alarm_list: Optional[EnumerationContextAlarmListType] = None


@dataclass(slots=True)
class EnumerationAlarmLevelType:
    """Describe an alarm level and its enumeration label to trigger from. See
    EnumeratedAlarmType and EnumeratedParameterType."""

    # attribute 'alarmLevel'
    alarm_level: ConcernLevelsType = ConcernLevelsType.NORMAL
    enumeration_label: str = ""  # attribute 'enumerationLabel'


@dataclass(slots=True)
class EnumerationAlarmListType:
    """XTCE type."""

    # element 'EnumerationAlarm'
    enumeration_alarm: list[
        EnumerationAlarmLevelType] = field(default_factory=list)


@dataclass(slots=True)
class EnumerationAlarmType:
    """Describe alarm conditions specific to the enumeration data type, extends
    the basic AlarmType with an EnumerationAlarmList. The alarms are described
    using the label (engineering/calibrated value) of the enumerated parameter.
    Enumeration labels may represent several raw/uncalibrated values, so as a
    result, a single alarm definition here may represent multiple raw values in
    the enumerated parameter. It is not necessary to define an alarm for
    raw/uncalibrated values that do not map to an enumeration. Implementations
    should implicitly define this as an alarm case, of which the manifestation
    of that is program/implementation specific. See EnumeratedParameterType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # attribute 'defaultAlarmLevel', schema default 'normal'
    default_alarm_level: Optional[ConcernLevelsType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[EnumerationAlarmChoice] = None
    # element 'EnumerationAlarmList'
    enumeration_alarm_list: Optional[EnumerationAlarmListType] = None


@dataclass(slots=True)
class EnumerationContextAlarmListType:
    """Describe an ordered collection of context enumeration alarms, duplicates
    are valid. Process the contexts in list order. See
    EnumerationContextAlarmType."""

    # element 'ContextAlarm'
    context_alarm: list[
        EnumerationContextAlarmType] = field(default_factory=list)


@dataclass(slots=True)
class EnumerationContextAlarmType:
    """Describe a context that when true the alarm condition may be evaluated.
    See ContextMatchType and EnumerationAlarmType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # attribute 'defaultAlarmLevel', schema default 'normal'
    default_alarm_level: Optional[ConcernLevelsType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[EnumerationContextAlarmChoice] = None
    # element 'EnumerationAlarmList'
    enumeration_alarm_list: Optional[EnumerationAlarmListType] = None
    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())


@dataclass(slots=True)
class EnumerationListType:
    """XTCE type."""

    # element 'Enumeration'
    enumeration: list[ValueEnumerationType] = field(default_factory=list)


@dataclass(slots=True)
class ErrorDetectCorrectType:
    """Describe CRC, Checksum, Parity, or XOR for error detection and correction
    algorithm calculation. See CRCType, ChecksumType, ParityType, and XORType.
    """

    items: list[ErrorDetectCorrectChoice] = field(default_factory=list)


@dataclass(slots=True)
class ExecutionVerifierType:
    """A verifier that indicates that the command is being executed. An optional
    Element indicates how far along the command has progressed either as a fixed
    value or an (possibly scaled) ParameterInstance value."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: ExecutionVerifierChoice1 = field(
        default_factory=lambda: ExecutionVerifierChoice1())
    choice2: ExecutionVerifierChoice2 = field(
        default_factory=lambda: ExecutionVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None
    # element 'PercentComplete'
    percent_complete: Optional[PercentCompleteType] = None


@dataclass(slots=True)
class ExternalAlgorithmSetType:
    """XTCE type."""

    # element 'ExternalAlgorithm'
    external_algorithm: list[
        ExternalAlgorithmType] = field(default_factory=list)


@dataclass(slots=True)
class ExternalAlgorithmType:
    """This is the external algorithm. Multiple entries are provided so that the
    same database may be used for multiple implementation s"""

    implementation_name: str = ""  # attribute 'implementationName'
    algorithm_location: str = ""  # attribute 'algorithmLocation'


@dataclass(slots=True)
class FailedVerifierType:
    """When true, indicates that the command failed. timeToWait is how long to
    wait for the FailedVerifier to test true."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: FailedVerifierChoice1 = field(
        default_factory=lambda: FailedVerifierChoice1())
    choice2: FailedVerifierChoice2 = field(
        default_factory=lambda: FailedVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None
    # element 'ReturnParmRef'
    return_parm_ref: Optional[ParameterRefType] = None


@dataclass(slots=True)
class FixedFrameStreamType:
    """For streams that contain a series of frames with a fixed frame length
    where the frames are found by looking for a marker in the data. This marker
    is sometimes called the frame sync pattern and sometimes the Asynchronous
    Sync Marker (ASM). This marker need not be contiguous although it usually
    is."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    bit_rate_in_bps: Optional[float] = None  # attribute 'bitRateInBPS'
    # attribute 'pcmType', schema default 'NRZL'
    pcm_type: Optional[PCMType] = None
    # attribute 'inverted', schema default 'false'
    inverted: Optional[bool] = None
    # attribute 'syncApertureInBits', schema default '0'
    sync_aperture_in_bits: Optional[int] = None
    frame_length_in_bits: int = 0  # attribute 'frameLengthInBits'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: FixedFrameStreamChoice = field(
        default_factory=lambda: FixedFrameStreamChoice())
    stream_ref: Optional[StreamRefType] = None  # element 'StreamRef'
    # element 'SyncStrategy'
    sync_strategy: FixedFrameSyncStrategyType = field(
        default_factory=lambda: FixedFrameSyncStrategyType())


@dataclass(slots=True)
class FixedFrameSyncStrategyType:
    """Describe a sync pattern and an optional reference to an algorithm used to
    invert the stream if the frame sync pattern is not found. See
    FixedFrameStreamType."""

    # attribute 'verifyToLockGoodFrames', schema default '4'
    verify_to_lock_good_frames: Optional[int] = None
    # attribute 'checkToLockGoodFrames', schema default '1'
    check_to_lock_good_frames: Optional[int] = None
    # attribute 'maxBitErrorsInSyncPattern', schema default '0'
    max_bit_errors_in_sync_pattern: Optional[int] = None
    auto_invert: Optional[AutoInvertType] = None  # element 'AutoInvert'
    # element 'SyncPattern'
    sync_pattern: SyncPatternType = field(
        default_factory=lambda: SyncPatternType())


@dataclass(slots=True)
class FlagType:
    """The pattern of bits used to look for frame synchronization."""

    # attribute 'flagSizeInBits', schema default '6'
    flag_size_in_bits: Optional[int] = None
    # attribute 'flagBitType', schema default 'ones'
    flag_bit_type: Optional[FlagBitType] = None


@dataclass(slots=True)
class FloatArgumentType:
    """Describe a floating point argument type. Several encodings are supported.
    Calibrated integer to float relationships should be described with this data
    type. Use the data encoding to define calibrators. Joins integer as one of
    the numerics. See FloatDataEncodingType, IntegerDataEncodingType and
    FloatDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[float] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[FloatSizeInBitsType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[FloatArgumentChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'
    # element 'ValidRangeSet'
    valid_range_set: Optional[ValidFloatRangeSetType] = None


@dataclass(slots=True)
class FloatDataEncodingType:
    """For common encodings of floating point data"""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # attribute 'encoding', schema default 'IEEE754_1985'
    encoding: Optional[FloatEncodingType] = None
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[FloatEncodingSizeInBitsType] = None
    change_threshold: Optional[float] = None  # attribute 'changeThreshold'
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    # element 'DefaultCalibrator'
    default_calibrator: Optional[CalibratorType] = None
    # element 'ContextCalibratorList'
    context_calibrator_list: Optional[ContextCalibratorListType] = None


@dataclass(slots=True)
class FloatDataType:
    """A base schema type for describing a floating point engineering/calibrated
    data type. Several encodings are supported. Calibrated integer to float
    relationships should be described with this data type. Use the data encoding
    to define calibrators. Joins integer as one of the numerics. See
    BaseDataType, FloatParameterType and FloatArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[float] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[FloatSizeInBitsType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[FloatDataChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'
    # element 'ValidRange'
    valid_range: Optional[FloatDataValidRangeType] = None


@dataclass(slots=True)
class FloatDataValidRangeType:
    """XTCE type."""

    min_inclusive: Optional[float] = None  # attribute 'minInclusive'
    min_exclusive: Optional[float] = None  # attribute 'minExclusive'
    max_inclusive: Optional[float] = None  # attribute 'maxInclusive'
    max_exclusive: Optional[float] = None  # attribute 'maxExclusive'
    # attribute 'validRangeAppliesToCalibrated', schema default 'true'
    valid_range_applies_to_calibrated: Optional[bool] = None


@dataclass(slots=True)
class FloatParameterType:
    """Describe a floating point parameter type. Several encodings are
    supported. Calibrated integer to float relationships should be described
    with this data type. Use the data encoding to define calibrators. Joins
    integer as one of the numerics. See FloatDataEncodingType,
    IntegerDataEncodingType and FloatDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[float] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[FloatSizeInBitsType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[FloatParameterChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'
    # element 'ValidRange'
    valid_range: Optional[FloatDataValidRangeType] = None
    default_alarm: Optional[NumericAlarmType] = None  # element 'DefaultAlarm'
    # element 'ContextAlarmList'
    context_alarm_list: Optional[NumericContextAlarmListType] = None


@dataclass(slots=True)
class FloatRangeType:
    """Describe a floating point based range, several types of ranges are
    supported -- one sided and two sided, inclusive or exclusive. It would not
    make sense to set two mins or maxes. Used in a number of locations related
    to ranges: ValidFloatRangeSetType or AlarmRangeType for example."""

    min_inclusive: Optional[float] = None  # attribute 'minInclusive'
    min_exclusive: Optional[float] = None  # attribute 'minExclusive'
    max_inclusive: Optional[float] = None  # attribute 'maxInclusive'
    max_exclusive: Optional[float] = None  # attribute 'maxExclusive'


@dataclass(slots=True)
class FrameStreamType:
    """The top level type definition for all data streams that are frame based.
    """

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    bit_rate_in_bps: Optional[float] = None  # attribute 'bitRateInBPS'
    # attribute 'pcmType', schema default 'NRZL'
    pcm_type: Optional[PCMType] = None
    # attribute 'inverted', schema default 'false'
    inverted: Optional[bool] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: FrameStreamChoice = field(
        default_factory=lambda: FrameStreamChoice())
    stream_ref: Optional[StreamRefType] = None  # element 'StreamRef'


@dataclass(slots=True)
class HeaderType:
    """Schema for a Header record. A header contains general information about
    the system or subsystem."""

    version: Optional[str] = None  # attribute 'version'
    date: Optional[str] = None  # attribute 'date'
    # attribute 'classification', schema default 'NotClassified'
    classification: Optional[str] = None
    # attribute 'classificationInstructions'
    classification_instructions: Optional[str] = None
    # attribute 'validationStatus'
    validation_status: ValidationStatusType = ValidationStatusType.UNKNOWN
    author_set: Optional[AuthorSetType] = None  # element 'AuthorSet'
    note_set: Optional[NoteSetType] = None  # element 'NoteSet'
    history_set: Optional[HistorySetType] = None  # element 'HistorySet'


@dataclass(slots=True)
class HistorySetType:
    """Describe an unordered collection of History elements. Usage is user
    defined. See HistoryType."""

    history: list[str] = field(default_factory=list)  # element 'History'


@dataclass(slots=True)
class IndirectParameterRefEntryType:
    """An entry whose name is given by the value of a ParamameterInstance. This
    entry may be used to implement dwell telemetry streams. The value of the
    parameter in ParameterInstance must use either the name of the Parameter or
    its alias. If it's an alias name, the alias namespace is supplied as an
    attribute."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    alias_name_space: Optional[str] = None  # attribute 'aliasNameSpace'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'ParameterInstance'
    parameter_instance: ParameterInstanceRefType = field(
        default_factory=lambda: ParameterInstanceRefType())


@dataclass(slots=True)
class InputAlgorithmType:
    """A set of labeled inputs is added to the SimpleAlgorithmType"""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'AlgorithmText'
    algorithm_text: Optional[AlgorithmTextType] = None
    # element 'ExternalAlgorithmSet'
    external_algorithm_set: Optional[ExternalAlgorithmSetType] = None
    input_set: Optional[InputSetType] = None  # element 'InputSet'


@dataclass(slots=True)
class InputOutputAlgorithmType:
    """A set of labeled outputs are added to the SimpleInputAlgorithmType"""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    thread: Optional[bool] = None  # attribute 'thread', schema default 'false'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'AlgorithmText'
    algorithm_text: Optional[AlgorithmTextType] = None
    # element 'ExternalAlgorithmSet'
    external_algorithm_set: Optional[ExternalAlgorithmSetType] = None
    input_set: Optional[InputSetType] = None  # element 'InputSet'
    output_set: Optional[OutputSetType] = None  # element 'OutputSet'


@dataclass(slots=True)
class InputOutputTriggerAlgorithmType:
    """Input output algorithm is extended with a set of labeled triggers. See
    InputOutputAlgorithmType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    thread: Optional[bool] = None  # attribute 'thread', schema default 'false'
    trigger_container: Optional[str] = None  # attribute 'triggerContainer'
    priority: Optional[int] = None  # attribute 'priority'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'AlgorithmText'
    algorithm_text: Optional[AlgorithmTextType] = None
    # element 'ExternalAlgorithmSet'
    external_algorithm_set: Optional[ExternalAlgorithmSetType] = None
    input_set: Optional[InputSetType] = None  # element 'InputSet'
    output_set: Optional[OutputSetType] = None  # element 'OutputSet'
    trigger_set: Optional[TriggerSetType] = None  # element 'TriggerSet'


@dataclass(slots=True)
class InputParameterInstanceRefType:
    """Names an input parameter to the algorithm. There are two attributes to
    InputParm, inputName and parameterName. parameterName is a parameter
    reference name for a parameter that will be used in this algorithm.
    inputName is an optional "friendly" name for the input parameter."""

    parameter_ref: str = ""  # attribute 'parameterRef'
    instance: Optional[int] = None  # attribute 'instance', schema default '0'
    # attribute 'useCalibratedValue', schema default 'true'
    use_calibrated_value: Optional[bool] = None
    input_name: Optional[str] = None  # attribute 'inputName'


@dataclass(slots=True)
class InputSetType:
    """XTCE type."""

    items: list[InputSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class IntegerArgumentType:
    """Describes an integer argument type. Several encodings supported.
    Calibrated integer to integer relationships should be described with this
    data type. Use the integer data encoding to define calibrators. Joins float
    as one of the numerics. See IntegerDataEncoding and IntegerDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[int] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[int] = None
    signed: Optional[bool] = None  # attribute 'signed', schema default 'true'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[IntegerArgumentChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'
    # element 'ValidRangeSet'
    valid_range_set: Optional[ValidIntegerRangeSetType] = None


@dataclass(slots=True)
class IntegerDataEncodingType:
    """For all major encodings of integer data"""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # attribute 'encoding', schema default 'unsigned'
    encoding: Optional[IntegerEncodingType] = None
    # attribute 'sizeInBits', schema default '8'
    size_in_bits: Optional[int] = None
    change_threshold: Optional[int] = None  # attribute 'changeThreshold'
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    # element 'DefaultCalibrator'
    default_calibrator: Optional[CalibratorType] = None
    # element 'ContextCalibratorList'
    context_calibrator_list: Optional[ContextCalibratorListType] = None


@dataclass(slots=True)
class IntegerDataType:
    """Describe an integer engineering/calibrated data type. Several encodings
    are supported. See BaseDataType, IntegerParameterType and
    IntegerArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[int] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[int] = None
    signed: Optional[bool] = None  # attribute 'signed', schema default 'true'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[IntegerDataChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'
    # element 'ValidRange'
    valid_range: Optional[IntegerDataValidRangeType] = None


@dataclass(slots=True)
class IntegerDataValidRangeType:
    """XTCE type."""

    min_inclusive: Optional[int] = None  # attribute 'minInclusive'
    max_inclusive: Optional[int] = None  # attribute 'maxInclusive'
    # attribute 'validRangeAppliesToCalibrated', schema default 'true'
    valid_range_applies_to_calibrated: Optional[bool] = None


@dataclass(slots=True)
class IntegerParameterType:
    """Describe an integer parameter type. Several are supported. Calibrated
    integer to integer relationships should be described with this data type.
    Use the integer data encoding to define calibrators. Joins float as one of
    the numerics. See IntegerDataEncoding and IntegerDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[int] = None  # attribute 'initialValue'
    # attribute 'sizeInBits', schema default '32'
    size_in_bits: Optional[int] = None
    signed: Optional[bool] = None  # attribute 'signed', schema default 'true'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[IntegerParameterChoice] = None
    to_string: Optional[ToStringType] = None  # element 'ToString'
    # element 'ValidRange'
    valid_range: Optional[IntegerDataValidRangeType] = None
    default_alarm: Optional[NumericAlarmType] = None  # element 'DefaultAlarm'
    # element 'ContextAlarmList'
    context_alarm_list: Optional[NumericContextAlarmListType] = None


@dataclass(slots=True)
class IntegerRangeType:
    """Describe an integral based range: minInclusive and maxInclusive. Used in
    a number of locations related to ranges: ValidIntegerRangeSetType for
    example."""

    min_inclusive: Optional[int] = None  # attribute 'minInclusive'
    max_inclusive: Optional[int] = None  # attribute 'maxInclusive'


@dataclass(slots=True)
class IntegerValueType:
    """Contains an Integer value; value may be provided directly or via the
    value in a parameter."""

    choice: IntegerValueChoice = field(
        default_factory=lambda: IntegerValueChoice())


@dataclass(slots=True)
class InterlockType:
    """Describe a type of constraint on the next command, rather than this
    command. Interlocks apply only to the next command. An interlock will block
    successive commands until this command has reached a certain stage of
    verifier. Interlocks are scoped to a SpaceSystem basis: they by default
    apply to the SpaceSystem the MetaCommand is defined in but this may be
    overridden. See MetaCommandType and VerifierSetType."""

    # attribute 'scopeToSpaceSystem'
    scope_to_space_system: Optional[str] = None
    # attribute 'verificationToWaitFor', schema default 'complete'
    verification_to_wait_for: Optional[VerifierEnumerationType] = None
    # attribute 'verificationProgressPercentage'
    verification_progress_percentage: Optional[float] = None
    # attribute 'suspendable', schema default 'false'
    suspendable: Optional[bool] = None


@dataclass(slots=True)
class LeadingSizeType:
    """Like PASCAL strings, the size of the string is given as an integer at the
    start of the string. SizeTag must be an unsigned Integer"""

    # attribute 'sizeInBitsOfSizeTag', schema default '16'
    size_in_bits_of_size_tag: Optional[int] = None


@dataclass(slots=True)
class LinearAdjustmentType:
    """A slope and intercept may be applied to scale or shift the value of the
    parameter in the dynamic value. The default of slope=1 and intercept=0
    results in no change to the value."""

    slope: Optional[float] = None  # attribute 'slope', schema default '1'
    # attribute 'intercept', schema default '0'
    intercept: Optional[float] = None


@dataclass(slots=True)
class LocationInContainerInBitsType:
    """Describe the absolute or relative bit location of an entry in a
    container. The "referenceLocation" attribute specifies the starting bit
    anchor. If no referenceLocation value is given, the entry is assumed to
    begin at the first bit position after the previous entry. Each container
    starts at bit 0, thus "containerStart" is an offset from 0. Negative
    container start bits are before the container and are implementation
    dependent - these should be flagged as likely errors. "containerEnd" is
    given as a positive offset from the end of the container, thus a container
    end of 0 is exactly at the end of the container. Negative container end
    addresses are after the container and are implementation dependent - these
    should be flagged as likely errors. Positive "previouEntry" values are
    offsets from the previous entry - zero (0) is the default which means it
    follows contiguously from the last occupied bit of the previous entry. A
    value of one means it is offset 1-bit from the previous entry, and a value
    of negative 1 (-1) means it overlaps the previous entry by one bit, and so
    forth. The "nextEntry" attribute value is proposed for deprecation and
    should be avoided. See SequenceEntryType."""

    # attribute 'referenceLocation', schema default 'previousEntry'
    reference_location: Optional[ReferenceLocationType] = None
    choice: LocationInContainerInBitsChoice = field(
        default_factory=lambda: LocationInContainerInBitsChoice())


@dataclass(slots=True)
class MatchCriteriaType:
    """Contains either a simple Comparison, a ComparisonList, an arbitrarily
    complex BooleanExpression or an escape to an externally defined algorithm"""

    choice: MatchCriteriaChoice = field(
        default_factory=lambda: MatchCriteriaChoice())


@dataclass(slots=True)
class MathAlgorithmType:
    """Describe a postfix (Reverse Polish Notation (RPN)) notation based
    mathmatical equations. See MathOperationType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'MathOperation'
    math_operation: TriggeredMathOperationType = field(
        default_factory=lambda: TriggeredMathOperationType())


@dataclass(slots=True)
class MathOperationCalibratorType:
    """Describe a mathematical function for calibration where the mathematical
    function is defined using the MathOperationType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    items: list[MathOperationCalibratorChoice] = field(default_factory=list)


@dataclass(slots=True)
class MathOperationType:
    """Postfix (aka Reverse Polish Notation (RPN)) notation is used to describe
    mathmatical equations. It uses a stack where operands (either fixed values
    or ParameterInstances) are pushed onto the stack from first to last in the
    XML. As the operators are specified, each pops off operands as it evaluates
    them, and pushes the result back onto the stack. In this case postfix is
    used to avoid having to specify parenthesis. To convert from infix to
    postfix, use Dijkstra's "shunting yard" algorithm."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    items: list[MathOperationChoice] = field(default_factory=list)


@dataclass(slots=True)
class MemberListType:
    """Order is important only if the name of the AggregateParameter or
    Aggregate Argument is directly referenced in SequenceContainers. In this
    case the members are assued to be added sequentially (in the order listed
    here) into the Container."""

    member: list[MemberType] = field(default_factory=list)  # element 'Member'


@dataclass(slots=True)
class MemberType:
    """Describe a member field in an AggregateDataType. Each member has a name
    and a type reference to a data type for the aggregate member name. If this
    aggregate is a Parameter aggregate, then the typeRef is a parameter type
    reference. If this aggregate is an Argument aggregate, then the typeRef is
    an argument type reference. References to an array data type is currently
    not supported. Circular references are not allowed. See MemberListType.
    AggregateParameterType and AggregateArgumentType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    type_ref: str = ""  # attribute 'typeRef'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class MessageRefSetType:
    """XTCE type."""

    # element 'MessageRef'
    message_ref: list[MessageRefType] = field(default_factory=list)


@dataclass(slots=True)
class MessageRefType:
    """Holds a reference to a message"""

    message_ref: str = ""  # attribute 'messageRef'


@dataclass(slots=True)
class MessageSetType:
    """XTCE type."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'Message'
    message: list[MessageType] = field(default_factory=list)


@dataclass(slots=True)
class MessageType:
    """XTCE type."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'MatchCriteria'
    match_criteria: MatchCriteriaType = field(
        default_factory=lambda: MatchCriteriaType())
    # element 'ContainerRef'
    container_ref: ContainerRefType = field(
        default_factory=lambda: ContainerRefType())


@dataclass(slots=True)
class MetaCommandSetType:
    """Describes an unordered collection of command definitions. Duplicates are
    invalid based on the name attribute of MetaCommand and BlockMetaCommand. See
    MetaCommandType and BlockMetaCommandType."""

    items: list[MetaCommandSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class MetaCommandStepListType:
    """Describe the list of MetaCommand definitions that form the block command.
    Contains an ordered list of MetaCommandSteps where each step is a
    MetaCommand with associated arguments, duplicates are valid. See
    BlockMetaCommandType."""

    # element 'MetaCommandStep'
    meta_command_step: list[MetaCommandStepType] = field(default_factory=list)


@dataclass(slots=True)
class MetaCommandStepType:
    """Describe a MetaCommand step, consisting MetaCommand reference and
    argument list. See MetaCommandStepListType and NameReferenceType."""

    meta_command_ref: str = ""  # attribute 'metaCommandRef'
    # element 'ArgumentAssignmentList'
    argument_assignment_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class MetaCommandType:
    """Describe a command which consists of an abstract portion (MetaCommand)
    and an optional packaging portion (MetaCommand CommandContainer). An
    argument list is provided. MetaCommand may extend other MetaCommands and
    their CommandContainer may extend other CommandContainer or
    SequenceContainers. A MetaCommand's CommandContainer is private except as
    referred to in BaseMetaCommand (they are not visible to other containers and
    cannot be used in an entry list). MetaCommands may also define various other
    behavioral aspects of a command such as command verifiers. See
    CommandContainerType, ArgumentListType, BaseMetaCommandType and
    BaseContainerType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    # attribute 'abstract', schema default 'false'
    abstract: Optional[bool] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'BaseMetaCommand'
    base_meta_command: Optional[BaseMetaCommandType] = None
    system_name: Optional[str] = None  # element 'SystemName'
    argument_list: Optional[ArgumentListType] = None  # element 'ArgumentList'
    # element 'CommandContainer'
    command_container: Optional[CommandContainerType] = None
    # element 'TransmissionConstraintList'
    transmission_constraint_list: Optional[
        TransmissionConstraintListType] = None
    # element 'DefaultSignificance'
    default_significance: Optional[SignificanceType] = None
    # element 'ContextSignificanceList'
    context_significance_list: Optional[ContextSignificanceListType] = None
    interlock: Optional[InterlockType] = None  # element 'Interlock'
    verifier_set: Optional[VerifierSetType] = None  # element 'VerifierSet'
    # element 'ParameterToSetList'
    parameter_to_set_list: Optional[ParameterToSetListType] = None
    # element 'ParametersToSuspendAlarmsOnSet'
    parameters_to_suspend_alarms_on_set: Optional[
        ParametersToSuspendAlarmsOnSetType] = None


@dataclass(slots=True)
class MultiRangeType:
    """The alarm multi-range element type permits users to define multiple alarm
    ranges in a sequence that goes beyond the more typical "inside" and
    "outside" range definitions. It can be thought of as a "barber pole"
    definition."""

    min_inclusive: Optional[float] = None  # attribute 'minInclusive'
    min_exclusive: Optional[float] = None  # attribute 'minExclusive'
    max_inclusive: Optional[float] = None  # attribute 'maxInclusive'
    max_exclusive: Optional[float] = None  # attribute 'maxExclusive'
    # attribute 'rangeForm', schema default 'outside'
    range_form: Optional[RangeFormType] = None
    level: Optional[ConcernLevelsType] = None  # attribute 'level'


@dataclass(slots=True)
class NameDescriptionType:
    """Defines a base schema type definition used by many other schema types
    throughout schema. Use it to describe a name with optional descriptions,
    aliases, and ancillary data. See NameType, LongDescriptionType,
    ShortDescriptionType, AliasSetType and AncillaryDataSetType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class NoteSetType:
    """Contains an unordered collection of Notes. Usage is user defined. See
    NoteType."""

    note: list[str] = field(default_factory=list)  # element 'Note'


@dataclass(slots=True)
class NumberFormatType:
    """This type describes how a numeric value should be represented in
    engineering/calibrated form. The defaults reflect the most common form."""

    # attribute 'numberBase', schema default 'Decimal'
    number_base: Optional[RadixType] = None
    # attribute 'minimumFractionDigits', schema default '0'
    minimum_fraction_digits: Optional[int] = None
    # attribute 'maximumFractionDigits'
    maximum_fraction_digits: Optional[int] = None
    # attribute 'minimumIntegerDigits', schema default '1'
    minimum_integer_digits: Optional[int] = None
    # attribute 'maximumIntegerDigits'
    maximum_integer_digits: Optional[int] = None
    # attribute 'negativeSuffix', schema default ''
    negative_suffix: Optional[str] = None
    # attribute 'positiveSuffix', schema default ''
    positive_suffix: Optional[str] = None
    # attribute 'negativePrefix', schema default '-'
    negative_prefix: Optional[str] = None
    # attribute 'positivePrefix', schema default ''
    positive_prefix: Optional[str] = None
    # attribute 'showThousandsGrouping', schema default 'false'
    show_thousands_grouping: Optional[bool] = None
    # attribute 'notation', schema default 'normal'
    notation: Optional[FloatingPointNotationType] = None


@dataclass(slots=True)
class NumericAlarmType:
    """Describe alarm conditions specific to the numeric data types, extends the
    basic AlarmType with StaticAlarmRanges and ChangeAlarmRanges. See
    FloatParameterType and IntegerParameterType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[NumericAlarmChoice] = None
    # element 'StaticAlarmRanges'
    static_alarm_ranges: Optional[AlarmRangesType] = None
    # element 'ChangeAlarmRanges'
    change_alarm_ranges: Optional[ChangeAlarmRangesType] = None
    # element 'AlarmMultiRanges'
    alarm_multi_ranges: Optional[AlarmMultiRangesType] = None


@dataclass(slots=True)
class NumericContextAlarmListType:
    """An ordered collection of numeric alarms associated with a context. A
    context is an alarm definition on a parameter which is valid only in the
    case of a test on the value of other parameters. Process the contexts in
    list order. Used by both FloatParameterType and IntegerParameterType. See
    NumericContextAlarmType."""

    # element 'ContextAlarm'
    context_alarm: list[NumericContextAlarmType] = field(default_factory=list)


@dataclass(slots=True)
class NumericContextAlarmType:
    """Describe a parameter dependent context, that when evaluates to true,
    enables the use of this alarm definition. See ContextMatchType and
    NumericAlarmType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[NumericContextAlarmChoice] = None
    # element 'StaticAlarmRanges'
    static_alarm_ranges: Optional[AlarmRangesType] = None
    # element 'ChangeAlarmRanges'
    change_alarm_ranges: Optional[ChangeAlarmRangesType] = None
    # element 'AlarmMultiRanges'
    alarm_multi_ranges: Optional[AlarmMultiRangesType] = None
    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())


@dataclass(slots=True)
class ORedConditionsType:
    """Describe two or more conditions that are logically ored together.
    Conditions may be a mix of Condition and ANDedCondition. See
    ORedConditionType and BooleanExpressionType."""

    items: list[ORedConditionsChoice] = field(default_factory=list)


@dataclass(slots=True)
class OnContainerUpdateTriggerType:
    """Describe a reference to container that triggers an event when the
    telemetry container referred to is updated (processed). See TriggerSetType.
    """

    container_ref: str = ""  # attribute 'containerRef'


@dataclass(slots=True)
class OnParameterUpdateTriggerType:
    """Describe a reference to parameter that triggers an event when the
    telemetry parameter referred to is updated (processed) with a new value. See
    TriggerSetType."""

    parameter_ref: str = ""  # attribute 'parameterRef'


@dataclass(slots=True)
class OnPeriodicRateTriggerType:
    """Describe a periodic time basis to trigger an event. See TriggerSetType.
    """

    fire_rate_in_seconds: float = 0.0  # attribute 'fireRateInSeconds'


@dataclass(slots=True)
class OptionalNameDescriptionType:
    """The type definition used by most elements that have an optional name with
    optional descriptions."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class OutputParameterRefType:
    """Names an output parameter to the algorithm. There are two attributes to
    OutputParm, outputName and parameterName. parameterName is a parameter
    reference name for a parameter that will be updated by this algorithm.
    outputName is an optional "friendly" name for the output parameter."""

    parameter_ref: str = ""  # attribute 'parameterRef'
    output_name: Optional[str] = None  # attribute 'outputName'


@dataclass(slots=True)
class OutputSetType:
    """XTCE type."""

    # element 'OutputParameterRef'
    output_parameter_ref: list[
        OutputParameterRefType] = field(default_factory=list)


@dataclass(slots=True)
class PCMStreamType:
    """A PCM Stream Type is the high level definition for all Pulse Code
    Modulated (PCM) (i.e., binary) streams."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    bit_rate_in_bps: Optional[float] = None  # attribute 'bitRateInBPS'
    # attribute 'pcmType', schema default 'NRZL'
    pcm_type: Optional[PCMType] = None
    # attribute 'inverted', schema default 'false'
    inverted: Optional[bool] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ParameterInstanceRefType:
    """A reference to an instance of a Parameter. Used when the value of a
    parameter is required for a calculation or as an index value. A positive
    value for instance is forward in time, a negative value for count is
    backward in time, a 0 value for count means use the current value of the
    parameter or the first value in a container."""

    parameter_ref: str = ""  # attribute 'parameterRef'
    instance: Optional[int] = None  # attribute 'instance', schema default '0'
    # attribute 'useCalibratedValue', schema default 'true'
    use_calibrated_value: Optional[bool] = None


@dataclass(slots=True)
class ParameterPropertiesType:
    """Describes extended properties/attributes of Parameter definitions."""

    # attribute 'dataSource'
    data_source: Optional[TelemetryDataSourceType] = None
    # attribute 'readOnly', schema default 'false'
    read_only: Optional[bool] = None
    # attribute 'persistence', schema default 'true'
    persistence: Optional[bool] = None
    system_name: Optional[str] = None  # element 'SystemName'
    # element 'ValidityCondition'
    validity_condition: Optional[MatchCriteriaType] = None
    # element 'PhysicalAddressSet'
    physical_address_set: Optional[PhysicalAddressSetType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None


@dataclass(slots=True)
class ParameterRefEntryType:
    """An entry that is a single Parameter"""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    parameter_ref: str = ""  # attribute 'parameterRef'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ParameterRefType:
    """A reference to a Parameter. Uses Unix-like naming across the SpaceSystem
    Tree (e.g., SimpleSat/Bus/EPDS/BatteryOne/Voltage). To reference an
    individual member of an array use the zero based bracket notation commonly
    used in languages like C, C++, and Java."""

    parameter_ref: str = ""  # attribute 'parameterRef'


@dataclass(slots=True)
class ParameterSegmentRefEntryType:
    """An entry that is only a portion of a parameter value indicating that the
    entire parameter value must be assembled from other parameter segments. It
    is assumed that parameter segments happen sequentially in time, that is the
    first part if a telemetry parameter first, however (and there's always a
    however), if this is not the case the order of this parameter segment may be
    supplied with the order attribute where the first segment order="0"."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    parameter_ref: str = ""  # attribute 'parameterRef'
    order: Optional[int] = None  # attribute 'order'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ParameterSetType:
    """Describe an unordered collection of parameters where duplicates defined
    by the Parameter name attribute are invalid. The ParameterSet exists in both
    the TelemetryMetaData and the CommandMetaData element so that each may be
    built independently but from a single namespace. See TelemetryMetaDataType
    and CommandMetaDataType."""

    items: list[ParameterSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class ParameterToSetListType:
    """Parameters that are set with a new value after the command has been sent.
    Appended to the Base Command list"""

    # element 'ParameterToSet'
    parameter_to_set: list[ParameterToSetType] = field(default_factory=list)


@dataclass(slots=True)
class ParameterToSetType:
    """Sets a Parameter to a new value (either from a derivation or explicitly)
    after the command has been verified (all verifications have passed)."""

    parameter_ref: str = ""  # attribute 'parameterRef'
    # attribute 'setOnVerification', schema default 'complete'
    set_on_verification: Optional[VerifierEnumerationType] = None
    choice: ParameterToSetChoice = field(
        default_factory=lambda: ParameterToSetChoice())


@dataclass(slots=True)
class ParameterToSuspendAlarmsOnType:
    """Will suspend all Alarms associated with this Parameter for the given
    suspense time after the given verifier"""

    parameter_ref: str = ""  # attribute 'parameterRef'
    suspense_time: str = ""  # attribute 'suspenseTime'
    # attribute 'verifierToTriggerOn', schema default 'release'
    verifier_to_trigger_on: Optional[VerifierEnumerationType] = None


@dataclass(slots=True)
class ParameterType:
    """Describe the properties of a telemetry parameter, including its data type
    (parameter type). The bulk of properties associated with a telemetry
    parameter are in its parameter type. The initial value specified here,
    overrides the initial value in the parameter type. A parameter may be local,
    in which case its parameter type would have no data encodings. Ideally such
    a definition would also set data source in parameter properties to "local"
    but the syntax does not enforce this. See BaseDataType, BaseTimeDataType,
    and NameReferenceType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    parameter_type_ref: str = ""  # attribute 'parameterTypeRef'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'ParameterProperties'
    parameter_properties: Optional[ParameterPropertiesType] = None


@dataclass(slots=True)
class ParameterTypeSetType:
    """Describe an unordered collection of parameter type definitions. These
    types named for the engineering/calibrated type of the parameter. See
    BaseDataType and BaseTimeDataType."""

    items: list[ParameterTypeSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class ParameterValueChangeType:
    """A parameter change in value or specified delta change in value."""

    # element 'ParameterRef'
    parameter_ref: ParameterRefType = field(
        default_factory=lambda: ParameterRefType())
    # element 'Change'
    change: ChangeValueType = field(default_factory=lambda: ChangeValueType())


@dataclass(slots=True)
class ParametersToSuspendAlarmsOnSetType:
    """Sometimes it is necessary to suspend alarms - particularly 'change'
    alarms for commands that will change the value of a Parameter"""

    # element 'ParameterToSuspendAlarmsOn'
    parameter_to_suspend_alarms_on: list[
        ParameterToSuspendAlarmsOnType] = field(default_factory=list)


@dataclass(slots=True)
class ParityType:
    """Describe the parity value. See ErrorDetectCorrectType."""

    type: ParityFormType = ParityFormType.EVEN  # attribute 'type'
    # attribute 'bitsFromReference', schema default '0'
    bits_from_reference: Optional[int] = None
    # attribute 'reference', schema default 'start'
    reference: Optional[ReferencePointType] = None
    parameter_ref: Optional[str] = None  # attribute 'parameterRef'


@dataclass(slots=True)
class PercentCompleteType:
    """Describe a percentage complete that is fixed from 0 to 100, or as value
    from a parameter. See ExecutionVerifierType."""

    choice: PercentCompleteChoice = field(
        default_factory=lambda: PercentCompleteChoice())


@dataclass(slots=True)
class PhysicalAddressSetType:
    """One or more physical addresses may be associated with each Parameter.
    Examples of physical addresses include a location on the spacecraft or a
    location on a data collection bus."""

    # element 'PhysicalAddress'
    physical_address: list[PhysicalAddressType] = field(default_factory=list)


@dataclass(slots=True)
class PhysicalAddressType:
    """Describe the physical address(s) that this parameter is collected from.
    Examples of physical addresses include a memory location on the spacecraft
    or a location on a data collection bus, with the source identified with a
    descriptive name for the region of memory, such as RAM, Flash, EEPROM, and
    other possibilities that can be adapted for program specific usage."""

    source_name: Optional[str] = None  # attribute 'sourceName'
    source_address: Optional[str] = None  # attribute 'sourceAddress'
    sub_address: Optional[PhysicalAddressType] = None  # element 'SubAddress'


@dataclass(slots=True)
class PolynomialCalibratorType:
    """Describe a polynomial equation for calibration. This is a calibration
    type where a curve in a raw vs calibrated plane is described using a set of
    polynomial coefficients. Raw values are converted to calibrated values by
    finding a position on the curve corresponding to the raw value. The first
    coefficient belongs with the X^0 term, the next coefficient belongs to the
    X^1 term and so on. See CalibratorType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    term: list[TermType] = field(default_factory=list)  # element 'Term'


@dataclass(slots=True)
class QueuedVerifierType:
    """A verifer that means the command is scheduled for execution by the
    destination."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: QueuedVerifierChoice1 = field(
        default_factory=lambda: QueuedVerifierChoice1())
    choice2: QueuedVerifierChoice2 = field(
        default_factory=lambda: QueuedVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class RateInStreamSetType:
    """XTCE type."""

    # element 'RateInStream'
    rate_in_stream: list[
        RateInStreamWithStreamNameType] = field(default_factory=list)


@dataclass(slots=True)
class RateInStreamType:
    """Define the expected appearance (rate) of a container in a stream where
    the rate is defined on either a perSecond or perContainer update basis. Many
    programs and platforms have variable reporting rates for containers and
    these can be commanded. As a result, this element is only useful to some
    users and generally does not affect the processing of the received
    containers themselves. See ContainerType."""

    # attribute 'basis', schema default 'perSecond'
    basis: Optional[BasisType] = None
    minimum_value: Optional[float] = None  # attribute 'minimumValue'
    maximum_value: Optional[float] = None  # attribute 'maximumValue'


@dataclass(slots=True)
class RateInStreamWithStreamNameType:
    """Define the expected appearance (rate) of a container in a named stream
    where the rate is defined on either a perSecond or perContainer update
    basis. Many programs and platforms have variable reporting rates for
    containers and these can be commanded. As a result, this element is only
    useful to some users and generally does not affect the processing of the
    received containers themselves. See ContainerType and RateInStreamType."""

    # attribute 'basis', schema default 'perSecond'
    basis: Optional[BasisType] = None
    minimum_value: Optional[float] = None  # attribute 'minimumValue'
    maximum_value: Optional[float] = None  # attribute 'maximumValue'
    stream_ref: str = ""  # attribute 'streamRef'


@dataclass(slots=True)
class ReceivedVerifierType:
    """A verifier that simply means the destination has received the command."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: ReceivedVerifierChoice1 = field(
        default_factory=lambda: ReceivedVerifierChoice1())
    choice2: ReceivedVerifierChoice2 = field(
        default_factory=lambda: ReceivedVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class ReferenceTimeType:
    """Most time values are relative to another time e.g. seconds are relative
    to minutes, minutes are relative to hours. This type is used to describe
    this relationship starting with the least significant time Parameter to and
    progressing to the most significant time parameter."""

    choice: ReferenceTimeChoice = field(
        default_factory=lambda: ReferenceTimeChoice())


@dataclass(slots=True)
class RelativeTimeArgumentType:
    """Describes a relative time argument type. Relative time parameters are
    time offsets (e.g. 10 second, 1.24 milliseconds, etc.) See
    IntegerDataEncodingType, FloatDataEncoding and RelativeTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class RelativeTimeDataType:
    """Used to contain a relative time value. Used to describe a relative time.
    Normally used for time offsets. A Relative time is expressed as PnYn MnDTnH
    nMnS, where nY represents the number of years, nM the number of months, nD
    the number of days, 'T' is the date/time separator, nH the number of hours,
    nM the number of minutes and nS the number of seconds. The number of seconds
    can include decimal digits to arbitrary precision. For example, to indicate
    a duration of 1 year, 2 months, 3 days, 10 hours, and 30 minutes, one would
    write: P1Y2M3DT10H30M. One could also indicate a duration of minus 120 days
    as: -P120D. An extension of Schema duration type."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None


@dataclass(slots=True)
class RelativeTimeParameterType:
    """Describes a relative time parameter type. Relative time parameters are
    time offsets (e.g. 10 second, 1.24 milliseconds, etc.) See
    IntegerDataEncodingType, FloatDataEncoding and RelativeTimeDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    encoding: Optional[EncodingType] = None  # element 'Encoding'
    # element 'ReferenceTime'
    reference_time: Optional[ReferenceTimeType] = None
    default_alarm: Optional[TimeAlarmType] = None  # element 'DefaultAlarm'
    # element 'ContextAlarmList'
    context_alarm_list: Optional[TimeContextAlarmListType] = None


@dataclass(slots=True)
class RepeatType:
    """Contains elements that describe how an Entry is identically repeated.
    This includes a Count of the number of appearances and an optional Offset in
    bits that may occur between appearances. A Count of 1 indicates no
    repetition. The Offset default is 0 when not specified."""

    # element 'Count'
    count: IntegerValueType = field(default_factory=lambda: IntegerValueType())
    offset: Optional[IntegerValueType] = None  # element 'Offset'


@dataclass(slots=True)
class RestrictionCriteriaType:
    """Define one or more conditions (constraints) for container inheritance. A
    container is instantiable if its constraints are true. Constraint conditions
    may be a comparison, a list of comparisons, a boolean expression, or a graph
    of containers that are instantiable (if all containers are instantiable the
    condition is true). See BaseContainerType, ComparisonType,
    ComparisonListType, BooleanExpressionType and NextContainerType."""

    choice: RestrictionCriteriaChoice = field(
        default_factory=lambda: RestrictionCriteriaChoice())
    next_container: Optional[ContainerRefType] = None  # element 'NextContainer'


@dataclass(slots=True)
class SentFromRangeVerifierType:
    """Sent from range means the command has been transmitted to the spacecraft
    by the network that connects the ground system to the spacecraft. Typically,
    this verifier would come from something other than the spacecraft, such as a
    modem or front end processor."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: SentFromRangeVerifierChoice1 = field(
        default_factory=lambda: SentFromRangeVerifierChoice1())
    choice2: SentFromRangeVerifierChoice2 = field(
        default_factory=lambda: SentFromRangeVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class SequenceContainerType:
    """Describes the binary layout/packing of data and also related properties,
    including an entry list of parameters, parameter segments, array parameters,
    stream segments, containers, and container segments. Sequence containers may
    extend other sequence containers (see BaseContainerType). The parent
    container's entries are placed before the entries in the child container
    forming one entry list. An inheritance chain may be formed using this
    mechanism, but only one entry list is being created. Sequence containers may
    be marked as "abstract", when this occurs an instance of it cannot itself be
    created. The idle pattern is part of any unallocated space in the container.
    See EntryListType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    # attribute 'abstract', schema default 'false'
    abstract: Optional[bool] = None
    # attribute 'idlePattern', schema default '0'
    idle_pattern: Optional[str] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'DefaultRateInStream'
    default_rate_in_stream: Optional[RateInStreamType] = None
    # element 'RateInStreamSet'
    rate_in_stream_set: Optional[RateInStreamSetType] = None
    # element 'BinaryEncoding'
    binary_encoding: Optional[ContainerBinaryDataEncodingType] = None
    # element 'EntryList'
    entry_list: EntryListType = field(default_factory=lambda: EntryListType())
    # element 'BaseContainer'
    base_container: Optional[BaseContainerType] = None


@dataclass(slots=True)
class SequenceEntryType:
    """Defines an abstract schema type used to create other entry types.
    Describe an entry's location in the container (See
    LocationInContainerInBitsType). The location may be fixed or dynamic,
    absolute or relative. Entries may be included depending on the value of a
    condition (See IncludeConditionType), and entries may also repeat (see
    RepeatEntryType). The entry's IncludeCondition resolves to true, it is
    fully-resolved when its size is computable after RepeatEntry has been
    accounted for and then offset by LocationInContainer. See EntryListType,
    IncludeConditionType, RepeatEntryType and LocationInContainerInBitsType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class ServiceRefType:
    """A reference to a Service"""

    service_ref: str = ""  # attribute 'serviceRef'


@dataclass(slots=True)
class ServiceSetType:
    """A service is a logical grouping of container and/or messages."""

    # element 'Service'
    service: list[ServiceType] = field(default_factory=list)


@dataclass(slots=True)
class ServiceType:
    """Holds a set of services, logical groups of containers OR messages (not
    both)."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: ServiceChoice = field(default_factory=lambda: ServiceChoice())


@dataclass(slots=True)
class SignificanceType:
    """Significance provides some cautionary information about the potential
    consequence of each MetaCommand."""

    space_system_at_risk: Optional[str] = None  # attribute 'spaceSystemAtRisk'
    reason_for_warning: Optional[str] = None  # attribute 'reasonForWarning'
    # attribute 'consequenceLevel', schema default 'normal'
    consequence_level: Optional[ConsequenceLevelType] = None


@dataclass(slots=True)
class SimpleAlgorithmType:
    """The simplest form of algorithm, a SimpleAlgorithmType contains an area
    for a free-form pseudo code description of the algorithm plus a Set of
    references to external algorithms. External algorithms are usually unique to
    a ground system type. Multiple external algorithms are possible because XTCE
    documents may be used across multiple ground systems."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'AlgorithmText'
    algorithm_text: Optional[AlgorithmTextType] = None
    # element 'ExternalAlgorithmSet'
    external_algorithm_set: Optional[ExternalAlgorithmSetType] = None


@dataclass(slots=True)
class SizeInBitsFixedType:
    """XTCE type."""

    fixed_value: int = 0  # element 'FixedValue'


@dataclass(slots=True)
class SizeInBitsType:
    """XTCE type."""

    # element 'Fixed'
    fixed: SizeInBitsFixedType = field(
        default_factory=lambda: SizeInBitsFixedType())
    termination_char: Optional[str] = None  # element 'TerminationChar'
    leading_size: Optional[LeadingSizeType] = None  # element 'LeadingSize'


@dataclass(slots=True)
class SpaceSystemType:
    """SpaceSystem is a collection of SpaceSystem(s) including space assets,
    ground assets, multi-satellite systems and sub-systems. A SpaceSystem is the
    root element for the set of data necessary to monitor and command an
    arbitrary space device - this includes the binary decomposition the data
    streams going into and out of a device."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    # attribute 'systemType', schema default 'unknown'
    system_type: Optional[SystemTypeType] = None
    # attribute 'assetType', schema default 'unknown'
    asset_type: Optional[str] = None
    operational_status: Optional[str] = None  # attribute 'operationalStatus'
    xml_base: Optional[str] = None  # attribute 'xml:base'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    header: Optional[HeaderType] = None  # element 'Header'
    # element 'TelemetryMetaData'
    telemetry_meta_data: Optional[TelemetryMetaDataType] = None
    # element 'CommandMetaData'
    command_meta_data: Optional[CommandMetaDataType] = None
    service_set: Optional[ServiceSetType] = None  # element 'ServiceSet'
    # element 'SpaceSystem'
    space_system: list[SpaceSystemType] = field(default_factory=list)


@dataclass(slots=True)
class SplineCalibratorType:
    """Describe a spline function for calibration using a set of at least 2
    points. Raw values are converted to calibrated values by finding a position
    on the line corresponding to the raw value. The line may be interpolated
    and/or extrapolated as needed. The interpolation order may be specified for
    all the points and overridden on individual points. The algorithm triggers
    on the input parameter. See CalibratorType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    order: Optional[int] = None  # attribute 'order', schema default '1'
    # attribute 'extrapolate', schema default 'false'
    extrapolate: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    # element 'SplinePoint'
    spline_point: list[SplinePointType] = field(default_factory=list)


@dataclass(slots=True)
class SplinePointType:
    """A spline, or piecewise defined function, is a set on points from which a
    curve may be drawn to interpolate raw to calibrated values"""

    order: Optional[int] = None  # attribute 'order', schema default '1'
    raw: float = 0.0  # attribute 'raw'
    calibrated: float = 0.0  # attribute 'calibrated'


@dataclass(slots=True)
class StreamRefType:
    """Holds a reference to a stream"""

    stream_ref: str = ""  # attribute 'streamRef'


@dataclass(slots=True)
class StreamSegmentEntryType:
    """An entry that is a portion of a stream (streams are by definition,
    assumed continuous) It is assumed that stream segments happen sequentially
    in time, that is the first part if a steam first, however, if this is not
    the case the order of the stream segments may be supplied with the order
    attribute where the first segment order="0"."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    stream_ref: str = ""  # attribute 'streamRef'
    order: Optional[int] = None  # attribute 'order'
    size_in_bits: int = 0  # attribute 'sizeInBits'
    # element 'LocationInContainerInBits'
    location_in_container_in_bits: Optional[
        LocationInContainerInBitsType] = None
    repeat_entry: Optional[RepeatType] = None  # element 'RepeatEntry'
    # element 'IncludeCondition'
    include_condition: Optional[MatchCriteriaType] = None
    # element 'TimeAssociation'
    time_association: Optional[TimeAssociationType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None


@dataclass(slots=True)
class StreamSetType:
    """Contains an unordered set of Streams."""

    items: list[StreamSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class StringAlarmLevelType:
    """Describe a string alarm condition based on matching a regular expression.
    The level and regular expression are described. The specific implementation
    of the regular expression syntax is not specified in the schema at this
    time. See StringAlarmListType."""

    # attribute 'alarmLevel'
    alarm_level: ConcernLevelsType = ConcernLevelsType.NORMAL
    match_pattern: str = ""  # attribute 'matchPattern'


@dataclass(slots=True)
class StringAlarmListType:
    """Describe an ordered collection of string alarms, where duplicates are
    valid. Evaluate the alarms in list order. The first to evaluate to true
    takes precedence. See StringAlarmLevelType."""

    # element 'StringAlarm'
    string_alarm: list[StringAlarmLevelType] = field(default_factory=list)


@dataclass(slots=True)
class StringAlarmType:
    """Describe alarms specific to the string data type, extends the basic
    AlarmType, while adding a StringAlarmList and defaultAlarmLevel attribute.
    The string alarm list is evaluated in list order. See ConcernsLevelsType and
    StringAlarmListType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # attribute 'defaultAlarmLevel', schema default 'normal'
    default_alarm_level: Optional[ConcernLevelsType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[StringAlarmChoice] = None
    # element 'StringAlarmList'
    string_alarm_list: Optional[StringAlarmListType] = None


@dataclass(slots=True)
class StringArgumentType:
    """Describes a string parameter type. Three forms are supported: fixed
    length, variable length and variable length using a prefix. See
    StringDataEncodingType and StringDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    restriction_pattern: Optional[str] = None  # attribute 'restrictionPattern'
    # attribute 'characterWidth'
    character_width: Optional[CharacterWidthType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[StringArgumentChoice] = None
    # element 'SizeRangeInCharacters'
    size_range_in_characters: Optional[IntegerRangeType] = None


@dataclass(slots=True)
class StringContextAlarmListType:
    """Describe an ordered collection of context string alarms, duplicates are
    valid. Process the contexts in list order. See StringContextAlarmType."""

    # element 'ContextAlarm'
    context_alarm: list[StringContextAlarmType] = field(default_factory=list)


@dataclass(slots=True)
class StringContextAlarmType:
    """Describe a context that when true the alarm may be evaluated. See
    ContextMatchType and StringAlarmType."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # attribute 'defaultAlarmLevel', schema default 'normal'
    default_alarm_level: Optional[ConcernLevelsType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[StringContextAlarmChoice] = None
    # element 'StringAlarmList'
    string_alarm_list: Optional[StringAlarmListType] = None
    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())


@dataclass(slots=True)
class StringDataEncodingType:
    """Describe common encodings of string data: UTF-8 and UTF-16. See
    StringDataType."""

    # attribute 'bitOrder', schema default 'mostSignificantBitFirst'
    bit_order: Optional[BitOrderType] = None
    # attribute 'byteOrder', schema default 'mostSignificantByteFirst'
    byte_order: Optional[str] = None
    # attribute 'encoding', schema default 'UTF-8'
    encoding: Optional[StringEncodingType] = None
    # element 'ErrorDetectCorrect'
    error_detect_correct: Optional[ErrorDetectCorrectType] = None
    choice: StringDataEncodingChoice = field(
        default_factory=lambda: StringDataEncodingChoice())


@dataclass(slots=True)
class StringDataType:
    """Defines a base schema type for StringParameterType and
    StringArgumentType, adding initial value, restriction pattern, character
    width, and size range in characters. The initial value if set is the initial
    value of all instances of the child types. The restriction pattern is a
    regular expression enforcing the string value to this pattern. The character
    width is on the local data type side. And the size range in character
    restricts the character set. For telemetered values, if the restriction
    pattern of size range in character is not met, the item is invalid. See
    BaseDataType, StringParameterType, StringArgumentType, CharacterWidthType
    and IntegerRangeType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    restriction_pattern: Optional[str] = None  # attribute 'restrictionPattern'
    # attribute 'characterWidth'
    character_width: Optional[CharacterWidthType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[StringDataChoice] = None
    # element 'SizeRangeInCharacters'
    size_range_in_characters: Optional[IntegerRangeType] = None


@dataclass(slots=True)
class StringParameterType:
    """Describes a string parameter type. Three forms are supported: fixed
    length, variable length and variable length using a prefix. See
    StringDataEncodingType and StringDataType."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    base_type: Optional[str] = None  # attribute 'baseType'
    initial_value: Optional[str] = None  # attribute 'initialValue'
    restriction_pattern: Optional[str] = None  # attribute 'restrictionPattern'
    # attribute 'characterWidth'
    character_width: Optional[CharacterWidthType] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    unit_set: Optional[UnitSetType] = None  # element 'UnitSet'
    choice: Optional[StringParameterChoice] = None
    # element 'SizeRangeInCharacters'
    size_range_in_characters: Optional[IntegerRangeType] = None
    default_alarm: Optional[StringAlarmType] = None  # element 'DefaultAlarm'
    # element 'ContextAlarmList'
    context_alarm_list: Optional[StringContextAlarmListType] = None


@dataclass(slots=True)
class SyncPatternType:
    """The pattern of bits used to look for frame synchronization."""

    pattern: str = ""  # attribute 'pattern'
    # attribute 'bitLocationFromStartOfContainer', schema default '0'
    bit_location_from_start_of_container: Optional[int] = None
    mask: Optional[str] = None  # attribute 'mask'
    mask_length_in_bits: Optional[int] = None  # attribute 'maskLengthInBits'
    pattern_length_in_bits: int = 0  # attribute 'patternLengthInBits'


@dataclass(slots=True)
class SyncStrategyType:
    """A Sync Strategy specifies the strategy on how to find frames within a
    stream of PCM data. The sync strategy is based upon a state machine that
    begins in the 'Search' state until the first sync marker is found. Then it
    goes into the 'Verify' state until a specified number of successive good
    sync markers are found. Then, the state machine goes into the 'Lock' state,
    in the 'Lock' state frames are considered good. Should a sync marker be
    missed in the 'Lock' state, the state machine will transition into the
    'Check' state, if the next sync marker is where it's expected within a
    specified number of frames, then the state machine will transition back to
    the 'Lock' state, it not it will transition back to 'Search'."""

    # attribute 'verifyToLockGoodFrames', schema default '4'
    verify_to_lock_good_frames: Optional[int] = None
    # attribute 'checkToLockGoodFrames', schema default '1'
    check_to_lock_good_frames: Optional[int] = None
    # attribute 'maxBitErrorsInSyncPattern', schema default '0'
    max_bit_errors_in_sync_pattern: Optional[int] = None
    auto_invert: Optional[AutoInvertType] = None  # element 'AutoInvert'


@dataclass(slots=True)
class TelemetryMetaDataType:
    """All the data about telemetry is contained in TelemetryMetaData"""

    # element 'ParameterTypeSet'
    parameter_type_set: Optional[ParameterTypeSetType] = None
    parameter_set: Optional[ParameterSetType] = None  # element 'ParameterSet'
    container_set: Optional[ContainerSetType] = None  # element 'ContainerSet'
    message_set: Optional[MessageSetType] = None  # element 'MessageSet'
    stream_set: Optional[StreamSetType] = None  # element 'StreamSet'
    algorithm_set: Optional[AlgorithmSetType] = None  # element 'AlgorithmSet'


@dataclass(slots=True)
class TermType:
    """A term in a polynomial expression."""

    coefficient: float = 0.0  # attribute 'coefficient'
    exponent: int = 0  # attribute 'exponent'


@dataclass(slots=True)
class TimeAlarmRangesType:
    """XTCE type."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'rangeForm', schema default 'outside'
    range_form: Optional[RangeFormType] = None
    # attribute 'timeUnits', schema default 'seconds'
    time_units: Optional[TimeUnitsType] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    watch_range: Optional[FloatRangeType] = None  # element 'WatchRange'
    warning_range: Optional[FloatRangeType] = None  # element 'WarningRange'
    distress_range: Optional[FloatRangeType] = None  # element 'DistressRange'
    critical_range: Optional[FloatRangeType] = None  # element 'CriticalRange'
    severe_range: Optional[FloatRangeType] = None  # element 'SevereRange'


@dataclass(slots=True)
class TimeAlarmType:
    """Alarms associated with time data types"""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[TimeAlarmChoice] = None
    # element 'StaticAlarmRanges'
    static_alarm_ranges: Optional[TimeAlarmRangesType] = None
    # element 'ChangePerSecondAlarmRanges'
    change_per_second_alarm_ranges: Optional[TimeAlarmRangesType] = None


@dataclass(slots=True)
class TimeAssociationType:
    """Describes a time association consisting of an instance of an absolute
    time parameter (parameterRef) and this entry. Because telemetry parameter
    instances are oftentimes "time-tagged" with a timing signal either provided
    on the ground or on the space system. This data element allows one to
    specify which of possibly many AbsoluteTimeParameters to use to "time-tag"
    parameter instances with. See AbsoluteTimeParameterType."""

    parameter_ref: str = ""  # attribute 'parameterRef'
    instance: Optional[int] = None  # attribute 'instance', schema default '0'
    # attribute 'useCalibratedValue', schema default 'true'
    use_calibrated_value: Optional[bool] = None
    # attribute 'interpolateTime', schema default 'true'
    interpolate_time: Optional[bool] = None
    offset: Optional[float] = None  # attribute 'offset'
    # attribute 'unit', schema default 'seconds'
    unit: Optional[TimeAssociationUnitType] = None


@dataclass(slots=True)
class TimeContextAlarmListType:
    """An ordered collection of temporal alarms associated with a context. A
    context is an alarm definition on a parameter which is valid only in the
    case of a test on the value of other parameters. Process the contexts in
    list order. See TimeContextAlarmType."""

    # element 'ContextAlarm'
    context_alarm: list[TimeContextAlarmType] = field(default_factory=list)


@dataclass(slots=True)
class TimeContextAlarmType:
    """Context alarms are applied when the ContextMatch is true. Context alarms
    override Default alarms"""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    # attribute 'minViolations', schema default '1'
    min_violations: Optional[int] = None
    # attribute 'minConformance', schema default '1'
    min_conformance: Optional[int] = None
    # attribute 'disabled', schema default 'false'
    disabled: Optional[bool] = None
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: Optional[TimeContextAlarmChoice] = None
    # element 'StaticAlarmRanges'
    static_alarm_ranges: Optional[TimeAlarmRangesType] = None
    # element 'ChangePerSecondAlarmRanges'
    change_per_second_alarm_ranges: Optional[TimeAlarmRangesType] = None
    # element 'ContextMatch'
    context_match: ContextMatchType = field(
        default_factory=lambda: ContextMatchType())


@dataclass(slots=True)
class ToStringType:
    """XTCE type."""

    # element 'NumberFormat'
    number_format: NumberFormatType = field(
        default_factory=lambda: NumberFormatType())


@dataclass(slots=True)
class TransferredToRangeVerifierType:
    """Transferred to range means the command has been received to the network
    that connects the ground system to the spacecraft. Typically, this verifier
    would come from something other than the spacecraft, such as a modem or
    front end processor."""

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: Optional[str] = None  # attribute 'name'
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice1: TransferredToRangeVerifierChoice1 = field(
        default_factory=lambda: TransferredToRangeVerifierChoice1())
    choice2: TransferredToRangeVerifierChoice2 = field(
        default_factory=lambda: TransferredToRangeVerifierChoice2())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class TransmissionConstraintListType:
    """Appended to the TramsmissionConstraint List of the base command.
    Constraints are checked in order."""

    # element 'TransmissionConstraint'
    transmission_constraint: list[
        TransmissionConstraintType] = field(default_factory=list)


@dataclass(slots=True)
class TransmissionConstraintType:
    """A CommandTransmission constraint is used to check that the command can be
    run in the current operating mode and may block the transmission of the
    command if the constraint condition is true."""

    time_out: Optional[str] = None  # attribute 'timeOut'
    # attribute 'suspendable', schema default 'false'
    suspendable: Optional[bool] = None
    choice: TransmissionConstraintChoice = field(
        default_factory=lambda: TransmissionConstraintChoice())
    # element 'ArgumentRestrictionList'
    argument_restriction_list: Optional[ArgumentAssignmentListType] = None


@dataclass(slots=True)
class TriggerSetType:
    """A trigger is used to initiate the processing of some algorithm. A trigger
    may be based on an update of a Parameter, receipt of a Container, or on a
    time basis. Triggers may also have a maximum rate that limits how often the
    trigger can be invoked."""

    name: Optional[str] = None  # attribute 'name'
    # attribute 'triggerRate', schema default '1'
    trigger_rate: Optional[int] = None
    items: list[TriggerSetChoice] = field(default_factory=list)


@dataclass(slots=True)
class TriggeredMathOperationType:
    """XTCE type."""

    name: Optional[str] = None  # attribute 'name'
    short_description: Optional[str] = None  # attribute 'shortDescription'
    output_parameter_ref: str = ""  # attribute 'outputParameterRef'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    items: list[TriggeredMathOperationChoice] = field(default_factory=list)
    # element 'TriggerSet'
    trigger_set: TriggerSetType = field(
        default_factory=lambda: TriggerSetType())


@dataclass(slots=True)
class UnitSetType:
    """Describe an ordered collection of units that form a unit-expression.
    Units may be described for both calibrated/engineering values and also
    potentially uncalibrated/raw values. See UnitType."""

    unit: list[UnitType] = field(default_factory=list)  # element 'Unit'


@dataclass(slots=True)
class UnitType:
    """Describe the exponent, factor, form, and description for a unit. The unit
    itself is in element Unit in UnitSet. See UnitSetType. The attributes are
    optional because different programs use this element in different ways,
    depending on vendor support."""

    power: Optional[float] = None  # attribute 'power', schema default '1'
    factor: Optional[str] = None  # attribute 'factor', schema default '1'
    description: Optional[str] = None  # attribute 'description'
    # attribute 'form', schema default 'calibrated'
    form: Optional[UnitFormType] = None
    value: str = ""  # text content


@dataclass(slots=True)
class ValidFloatRangeSetType:
    """Numerical ranges that define the universe of valid values for this
    argument. A single range is the most common, although it is possible to
    define multiple ranges when the valid values are not contiguous."""

    # attribute 'validRangeAppliesToCalibrated', schema default 'true'
    valid_range_applies_to_calibrated: Optional[bool] = None
    # element 'ValidRange'
    valid_range: list[FloatRangeType] = field(default_factory=list)


@dataclass(slots=True)
class ValidIntegerRangeSetType:
    """Numerical ranges that define the universe of valid values for this
    argument. A single range is the most common, although it is possible to
    define multiple ranges when the valid values are not contiguous."""

    # attribute 'validRangeAppliesToCalibrated', schema default 'true'
    valid_range_applies_to_calibrated: Optional[bool] = None
    # element 'ValidRange'
    valid_range: list[IntegerRangeType] = field(default_factory=list)


@dataclass(slots=True)
class ValueEnumerationType:
    """Describe a value and an associated string label, see EnumerationListType.
    """

    value: int = 0  # attribute 'value'
    max_value: Optional[int] = None  # attribute 'maxValue'
    label: str = ""  # attribute 'label'
    short_description: Optional[str] = None  # attribute 'shortDescription'


@dataclass(slots=True)
class VariableFrameStreamType:
    """For streams that contain a series of frames with a variable frame length
    where the frames are found by looking for a series of one's or zero's
    (usually one's). The series is called the flag. in the PCM stream that are
    usually made to be illegal in the PCM stream by zero or one bit insertion.
    """

    short_description: Optional[str] = None  # attribute 'shortDescription'
    name: str = ""  # attribute 'name'
    bit_rate_in_bps: Optional[float] = None  # attribute 'bitRateInBPS'
    # attribute 'pcmType', schema default 'NRZL'
    pcm_type: Optional[PCMType] = None
    # attribute 'inverted', schema default 'false'
    inverted: Optional[bool] = None
    long_description: Optional[str] = None  # element 'LongDescription'
    alias_set: Optional[AliasSetType] = None  # element 'AliasSet'
    # element 'AncillaryDataSet'
    ancillary_data_set: Optional[AncillaryDataSetType] = None
    choice: VariableFrameStreamChoice = field(
        default_factory=lambda: VariableFrameStreamChoice())
    stream_ref: Optional[StreamRefType] = None  # element 'StreamRef'
    # element 'SyncStrategy'
    sync_strategy: VariableFrameSyncStrategyType = field(
        default_factory=lambda: VariableFrameSyncStrategyType())


@dataclass(slots=True)
class VariableFrameSyncStrategyType:
    """XTCE type."""

    # attribute 'verifyToLockGoodFrames', schema default '4'
    verify_to_lock_good_frames: Optional[int] = None
    # attribute 'checkToLockGoodFrames', schema default '1'
    check_to_lock_good_frames: Optional[int] = None
    # attribute 'maxBitErrorsInSyncPattern', schema default '0'
    max_bit_errors_in_sync_pattern: Optional[int] = None
    auto_invert: Optional[AutoInvertType] = None  # element 'AutoInvert'
    flag: FlagType = field(default_factory=lambda: FlagType())  # element 'Flag'


@dataclass(slots=True)
class VariableStringType:
    """Describe a variable string whose length may change between samples."""

    max_size_in_bits: int = 0  # attribute 'maxSizeInBits'
    choice1: Optional[VariableStringChoice1] = None
    choice2: VariableStringChoice2 = field(
        default_factory=lambda: VariableStringChoice2())


@dataclass(slots=True)
class VerifierSetType:
    """Describe a collection of unordered verifiers. A command verifier is a
    conditional check on the telemetry from a SpaceSystem that that provides
    positive indication on the processing state of a command. There are eight
    different verifiers each associated with different states in command
    processing: TransferredToRange, TransferredFromRange, Received, Accepted,
    Queued, Execution, Complete, and Failed. There may be multiple "complete"
    and "execution" verifiers. If the MetaCommand is part of an inheritance
    relation (BaseMetaCommand), the "complete" and "execution" verifier sets are
    appended to any defined in the parent MetaCommand. All others will override
    a verifier defined in a BaseMetaCommand. Duplicate verifiers in the list of
    CompleteVerifiers and ExecutionVerifiers before and after appending to the
    verifiers in BaseMetaCommand should be avoided. See MetaCommandType and
    BaseMetaCommandType for additional information."""

    # element 'TransferredToRangeVerifier'
    transferred_to_range_verifier: Optional[
        TransferredToRangeVerifierType] = None
    # element 'SentFromRangeVerifier'
    sent_from_range_verifier: Optional[SentFromRangeVerifierType] = None
    # element 'ReceivedVerifier'
    received_verifier: Optional[ReceivedVerifierType] = None
    # element 'AcceptedVerifier'
    accepted_verifier: Optional[AcceptedVerifierType] = None
    # element 'QueuedVerifier'
    queued_verifier: Optional[QueuedVerifierType] = None
    # element 'ExecutionVerifier'
    execution_verifier: list[
        ExecutionVerifierType] = field(default_factory=list)
    # element 'CompleteVerifier'
    complete_verifier: list[CompleteVerifierType] = field(default_factory=list)
    # element 'FailedVerifier'
    failed_verifier: Optional[FailedVerifierType] = None


@dataclass(slots=True)
class XORType:
    """Describe an exclusive or (XOR) checksum definition. See
    ErrorDetectCorrectType."""

    # attribute 'bitsFromReference', schema default '0'
    bits_from_reference: Optional[int] = None
    # attribute 'reference', schema default 'start'
    reference: Optional[ReferencePointType] = None
    parameter_ref: Optional[str] = None  # attribute 'parameterRef'


@dataclass(slots=True)
class ANDedConditionsChoice:
    """One alternative of a ``<choice>`` in :class:`ANDedConditionsType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Condition'
    value: Any = field(default_factory=lambda: ComparisonCheckType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'Condition', 'ORedConditions'})

    @classmethod
    def condition(cls, value: ComparisonCheckType) -> ANDedConditionsChoice:
        """Build the ``Condition`` alternative."""
        return cls('Condition', value)

    @classmethod
    def o_red_conditions(cls,
                         value: ORedConditionsType) -> ANDedConditionsChoice:
        """Build the ``ORedConditions`` alternative."""
        return cls('ORedConditions', value)


@dataclass(slots=True)
class AcceptedVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`AcceptedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> AcceptedVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> AcceptedVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> AcceptedVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> AcceptedVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> AcceptedVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> AcceptedVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class AcceptedVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`AcceptedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> AcceptedVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> AcceptedVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class AlarmChoice:
    """One alternative of a ``<choice>`` in :class:`AlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls, value: AlarmConditionsType) -> AlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> AlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class AlgorithmSetChoice:
    """One alternative of a ``<choice>`` in :class:`AlgorithmSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CustomAlgorithm'
    value: Any = field(
        default_factory=lambda: InputOutputTriggerAlgorithmType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CustomAlgorithm",
                                                 "MathAlgorithm"})

    @classmethod
    def custom_algorithm(
            cls, value: InputOutputTriggerAlgorithmType) -> AlgorithmSetChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def math_algorithm(cls, value: MathAlgorithmType) -> AlgorithmSetChoice:
        """Build the ``MathAlgorithm`` alternative."""
        return cls('MathAlgorithm', value)


@dataclass(slots=True)
class ArgumentANDedConditionsChoice:
    """One alternative of a ``<choice>`` in
    :class:`ArgumentANDedConditionsType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Condition'
    value: Any = field(default_factory=lambda: ArgumentComparisonCheckType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'Condition', 'ORedConditions'})

    @classmethod
    def condition(cls,
            value: ArgumentComparisonCheckType
            ) -> ArgumentANDedConditionsChoice:
        """Build the ``Condition`` alternative."""
        return cls('Condition', value)

    @classmethod
    def o_red_conditions(
            cls,
            value: ArgumentORedConditionsType) -> ArgumentANDedConditionsChoice:
        """Build the ``ORedConditions`` alternative."""
        return cls('ORedConditions', value)


@dataclass(slots=True)
class ArgumentBaseDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentBaseDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> ArgumentBaseDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentBaseDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> ArgumentBaseDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> ArgumentBaseDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentBinaryDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentBinaryDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> ArgumentBinaryDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentBinaryDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> ArgumentBinaryDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> ArgumentBinaryDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentBooleanDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentBooleanDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> ArgumentBooleanDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentBooleanDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> ArgumentBooleanDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> ArgumentBooleanDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentBooleanExpressionChoice:
    """One alternative of a ``<choice>`` in
    :class:`ArgumentBooleanExpressionType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Condition'
    value: Any = field(default_factory=lambda: ArgumentComparisonCheckType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Condition", "ANDedConditions",
                                                 "ORedConditions"})

    @classmethod
    def condition(cls,
            value: ArgumentComparisonCheckType
            ) -> ArgumentBooleanExpressionChoice:
        """Build the ``Condition`` alternative."""
        return cls('Condition', value)

    @classmethod
    def an_ded_conditions(cls,
            value: ArgumentANDedConditionsType
            ) -> ArgumentBooleanExpressionChoice:
        """Build the ``ANDedConditions`` alternative."""
        return cls('ANDedConditions', value)

    @classmethod
    def o_red_conditions(cls,
            value: ArgumentORedConditionsType
            ) -> ArgumentBooleanExpressionChoice:
        """Build the ``ORedConditions`` alternative."""
        return cls('ORedConditions', value)


@dataclass(slots=True)
class ArgumentComparisonCheckChoice1:
    """One alternative of a ``<choice>`` in
    :class:`ArgumentComparisonCheckType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ParameterInstanceRef'
    value: Any = field(default_factory=lambda: ParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"ParameterInstanceRef",
                                                 "ArgumentInstanceRef"})

    @classmethod
    def parameter_instance_ref(
            cls,
            value: ParameterInstanceRefType) -> ArgumentComparisonCheckChoice1:
        """Build the ``ParameterInstanceRef`` alternative."""
        return cls('ParameterInstanceRef', value)

    @classmethod
    def argument_instance_ref(
            cls,
            value: ArgumentInstanceRefType) -> ArgumentComparisonCheckChoice1:
        """Build the ``ArgumentInstanceRef`` alternative."""
        return cls('ArgumentInstanceRef', value)


@dataclass(slots=True)
class ArgumentComparisonCheckChoice2:
    """One alternative of a ``<choice>`` in
    :class:`ArgumentComparisonCheckType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ParameterInstanceRef'
    value: Any = field(default_factory=lambda: ParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ParameterInstanceRef", "ArgumentInstanceRef", "Value"})

    @classmethod
    def parameter_instance_ref(
            cls,
            value: ParameterInstanceRefType) -> ArgumentComparisonCheckChoice2:
        """Build the ``ParameterInstanceRef`` alternative."""
        return cls('ParameterInstanceRef', value)

    @classmethod
    def argument_instance_ref(
            cls,
            value: ArgumentInstanceRefType) -> ArgumentComparisonCheckChoice2:
        """Build the ``ArgumentInstanceRef`` alternative."""
        return cls('ArgumentInstanceRef', value)

    @classmethod
    def value_(cls, value: str) -> ArgumentComparisonCheckChoice2:
        """Build the ``Value`` alternative."""
        return cls('Value', value)


@dataclass(slots=True)
class ArgumentComparisonChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentComparisonType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ParameterInstanceRef'
    value: Any = field(default_factory=lambda: ParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"ParameterInstanceRef",
                                                 "ArgumentInstanceRef"})

    @classmethod
    def parameter_instance_ref(
            cls, value: ParameterInstanceRefType) -> ArgumentComparisonChoice:
        """Build the ``ParameterInstanceRef`` alternative."""
        return cls('ParameterInstanceRef', value)

    @classmethod
    def argument_instance_ref(
            cls, value: ArgumentInstanceRefType) -> ArgumentComparisonChoice:
        """Build the ``ArgumentInstanceRef`` alternative."""
        return cls('ArgumentInstanceRef', value)


@dataclass(slots=True)
class ArgumentDiscreteLookupChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentDiscreteLookupType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CustomAlgorithm'
    value: Any = field(default_factory=lambda: ArgumentInputAlgorithmType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(
            cls, value: ArgumentComparisonType) -> ArgumentDiscreteLookupChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(
            cls,
            value: ArgumentComparisonListType) -> ArgumentDiscreteLookupChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(cls,
            value: ArgumentBooleanExpressionType
            ) -> ArgumentDiscreteLookupChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(
            cls,
            value: ArgumentInputAlgorithmType) -> ArgumentDiscreteLookupChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class ArgumentDynamicValueChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentDynamicValueType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ArgumentInstanceRef'
    value: Any = field(default_factory=lambda: ArgumentInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"ArgumentInstanceRef",
                                                 "ParameterInstanceRef"})

    @classmethod
    def argument_instance_ref(
            cls, value: ArgumentInstanceRefType) -> ArgumentDynamicValueChoice:
        """Build the ``ArgumentInstanceRef`` alternative."""
        return cls('ArgumentInstanceRef', value)

    @classmethod
    def parameter_instance_ref(
            cls, value: ParameterInstanceRefType) -> ArgumentDynamicValueChoice:
        """Build the ``ParameterInstanceRef`` alternative."""
        return cls('ParameterInstanceRef', value)


@dataclass(slots=True)
class ArgumentEnumeratedDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentEnumeratedDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(cls,
            value: ArgumentBinaryDataEncodingType
            ) -> ArgumentEnumeratedDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentEnumeratedDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls,
            value: IntegerDataEncodingType) -> ArgumentEnumeratedDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(cls,
            value: ArgumentStringDataEncodingType
            ) -> ArgumentEnumeratedDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentFloatDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentFloatDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> ArgumentFloatDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentFloatDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> ArgumentFloatDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> ArgumentFloatDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentInputSetChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentInputSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'InputParameterInstanceRef'
    value: Any = field(default_factory=lambda: InputParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"InputParameterInstanceRef", "InputArgumentInstanceRef", "Constant"})

    @classmethod
    def input_parameter_instance_ref(
            cls,
            value: InputParameterInstanceRefType) -> ArgumentInputSetChoice:
        """Build the ``InputParameterInstanceRef`` alternative."""
        return cls('InputParameterInstanceRef', value)

    @classmethod
    def input_argument_instance_ref(
            cls, value: ArgumentInstanceRefType) -> ArgumentInputSetChoice:
        """Build the ``InputArgumentInstanceRef`` alternative."""
        return cls('InputArgumentInstanceRef', value)

    @classmethod
    def constant(cls, value: ConstantType) -> ArgumentInputSetChoice:
        """Build the ``Constant`` alternative."""
        return cls('Constant', value)


@dataclass(slots=True)
class ArgumentIntegerDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentIntegerDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> ArgumentIntegerDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentIntegerDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> ArgumentIntegerDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> ArgumentIntegerDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentIntegerValueChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentIntegerValueType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FixedValue'
    value: Any = 0
    NAMES: ClassVar[frozenset[str]] = frozenset({"FixedValue", "DynamicValue",
                                                 "DiscreteLookupList"})

    @classmethod
    def fixed_value(cls, value: int) -> ArgumentIntegerValueChoice:
        """Build the ``FixedValue`` alternative."""
        return cls('FixedValue', value)

    @classmethod
    def dynamic_value(
            cls, value: ArgumentDynamicValueType) -> ArgumentIntegerValueChoice:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)

    @classmethod
    def discrete_lookup_list(cls,
            value: ArgumentDiscreteLookupListType
            ) -> ArgumentIntegerValueChoice:
        """Build the ``DiscreteLookupList`` alternative."""
        return cls('DiscreteLookupList', value)


@dataclass(slots=True)
class ArgumentLocationInContainerInBitsChoice:
    """One alternative of a ``<choice>`` in
    :class:`ArgumentLocationInContainerInBitsType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FixedValue'
    value: Any = 0
    NAMES: ClassVar[frozenset[str]] = frozenset({"FixedValue", "DynamicValue",
                                                 "DiscreteLookupList"})

    @classmethod
    def fixed_value(cls, value: int) -> ArgumentLocationInContainerInBitsChoice:
        """Build the ``FixedValue`` alternative."""
        return cls('FixedValue', value)

    @classmethod
    def dynamic_value(cls,
            value: ArgumentDynamicValueType
            ) -> ArgumentLocationInContainerInBitsChoice:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)

    @classmethod
    def discrete_lookup_list(cls,
            value: ArgumentDiscreteLookupListType
            ) -> ArgumentLocationInContainerInBitsChoice:
        """Build the ``DiscreteLookupList`` alternative."""
        return cls('DiscreteLookupList', value)


@dataclass(slots=True)
class ArgumentMatchCriteriaChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentMatchCriteriaType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CustomAlgorithm'
    value: Any = field(default_factory=lambda: ArgumentInputAlgorithmType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(
            cls, value: ArgumentComparisonType) -> ArgumentMatchCriteriaChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(
            cls,
            value: ArgumentComparisonListType) -> ArgumentMatchCriteriaChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(cls,
            value: ArgumentBooleanExpressionType
            ) -> ArgumentMatchCriteriaChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(
            cls,
            value: ArgumentInputAlgorithmType) -> ArgumentMatchCriteriaChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class ArgumentMathOperationChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentMathOperationType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ValueOperand'
    value: Any = ""
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ValueOperand", "ThisParameterOperand", "Operator",
         "ParameterInstanceRefOperand", "ArgumentInstanceRefOperand"})

    @classmethod
    def value_operand(cls, value: str) -> ArgumentMathOperationChoice:
        """Build the ``ValueOperand`` alternative."""
        return cls('ValueOperand', value)

    @classmethod
    def this_parameter_operand(cls, value: str) -> ArgumentMathOperationChoice:
        """Build the ``ThisParameterOperand`` alternative."""
        return cls('ThisParameterOperand', value)

    @classmethod
    def operator(cls, value: MathOperatorsType) -> ArgumentMathOperationChoice:
        """Build the ``Operator`` alternative."""
        return cls('Operator', value)

    @classmethod
    def parameter_instance_ref_operand(
            cls,
            value: ParameterInstanceRefType) -> ArgumentMathOperationChoice:
        """Build the ``ParameterInstanceRefOperand`` alternative."""
        return cls('ParameterInstanceRefOperand', value)

    @classmethod
    def argument_instance_ref_operand(
            cls, value: ArgumentInstanceRefType) -> ArgumentMathOperationChoice:
        """Build the ``ArgumentInstanceRefOperand`` alternative."""
        return cls('ArgumentInstanceRefOperand', value)


@dataclass(slots=True)
class ArgumentORedConditionsChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentORedConditionsType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Condition'
    value: Any = field(default_factory=lambda: ArgumentComparisonCheckType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Condition",
                                                 "ANDedConditions"})

    @classmethod
    def condition(
            cls,
            value: ArgumentComparisonCheckType) -> ArgumentORedConditionsChoice:
        """Build the ``Condition`` alternative."""
        return cls('Condition', value)

    @classmethod
    def an_ded_conditions(
            cls,
            value: ArgumentANDedConditionsType) -> ArgumentORedConditionsChoice:
        """Build the ``ANDedConditions`` alternative."""
        return cls('ANDedConditions', value)


@dataclass(slots=True)
class ArgumentStringDataChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentStringDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> ArgumentStringDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> ArgumentStringDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> ArgumentStringDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> ArgumentStringDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class ArgumentStringDataEncodingChoice:
    """One alternative of a ``<choice>`` in
    :class:`ArgumentStringDataEncodingType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'SizeInBits'
    value: Any = field(default_factory=lambda: SizeInBitsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'SizeInBits', 'Variable'})

    @classmethod
    def size_in_bits(cls,
                     value: SizeInBitsType) -> ArgumentStringDataEncodingChoice:
        """Build the ``SizeInBits`` alternative."""
        return cls('SizeInBits', value)

    @classmethod
    def variable(cls,
            value: ArgumentVariableStringType
            ) -> ArgumentStringDataEncodingChoice:
        """Build the ``Variable`` alternative."""
        return cls('Variable', value)


@dataclass(slots=True)
class ArgumentTypeSetChoice:
    """One alternative of a ``<choice>`` in :class:`ArgumentTypeSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'StringArgumentType'
    value: Any = field(default_factory=lambda: StringArgumentType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"StringArgumentType", "EnumeratedArgumentType", "IntegerArgumentType",
         "BinaryArgumentType", "FloatArgumentType", "BooleanArgumentType",
         "RelativeTimeArgumentType", "AbsoluteTimeArgumentType",
         "ArrayArgumentType", "AggregateArgumentType"})

    @classmethod
    def string_argument_type(
            cls, value: StringArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``StringArgumentType`` alternative."""
        return cls('StringArgumentType', value)

    @classmethod
    def enumerated_argument_type(
            cls, value: EnumeratedArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``EnumeratedArgumentType`` alternative."""
        return cls('EnumeratedArgumentType', value)

    @classmethod
    def integer_argument_type(
            cls, value: IntegerArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``IntegerArgumentType`` alternative."""
        return cls('IntegerArgumentType', value)

    @classmethod
    def binary_argument_type(
            cls, value: BinaryArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``BinaryArgumentType`` alternative."""
        return cls('BinaryArgumentType', value)

    @classmethod
    def float_argument_type(cls,
                            value: FloatArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``FloatArgumentType`` alternative."""
        return cls('FloatArgumentType', value)

    @classmethod
    def boolean_argument_type(
            cls, value: BooleanArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``BooleanArgumentType`` alternative."""
        return cls('BooleanArgumentType', value)

    @classmethod
    def relative_time_argument_type(
            cls, value: RelativeTimeArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``RelativeTimeArgumentType`` alternative."""
        return cls('RelativeTimeArgumentType', value)

    @classmethod
    def absolute_time_argument_type(
            cls, value: AbsoluteTimeArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``AbsoluteTimeArgumentType`` alternative."""
        return cls('AbsoluteTimeArgumentType', value)

    @classmethod
    def array_argument_type(cls,
                            value: ArrayArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``ArrayArgumentType`` alternative."""
        return cls('ArrayArgumentType', value)

    @classmethod
    def aggregate_argument_type(
            cls, value: AggregateArgumentType) -> ArgumentTypeSetChoice:
        """Build the ``AggregateArgumentType`` alternative."""
        return cls('AggregateArgumentType', value)


@dataclass(slots=True)
class ArgumentVariableStringChoice1:
    """One alternative of a ``<choice>`` in :class:`ArgumentVariableStringType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'DynamicValue'
    value: Any = field(default_factory=lambda: ArgumentDynamicValueType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"DynamicValue",
                                                 "DiscreteLookupList"})

    @classmethod
    def dynamic_value(
            cls,
            value: ArgumentDynamicValueType) -> ArgumentVariableStringChoice1:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)

    @classmethod
    def discrete_lookup_list(cls,
            value: ArgumentDiscreteLookupListType
            ) -> ArgumentVariableStringChoice1:
        """Build the ``DiscreteLookupList`` alternative."""
        return cls('DiscreteLookupList', value)


@dataclass(slots=True)
class ArgumentVariableStringChoice2:
    """One alternative of a ``<choice>`` in :class:`ArgumentVariableStringType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'LeadingSize'
    value: Any = field(default_factory=lambda: LeadingSizeType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"LeadingSize",
                                                 "TerminationChar"})

    @classmethod
    def leading_size(cls,
                     value: LeadingSizeType) -> ArgumentVariableStringChoice2:
        """Build the ``LeadingSize`` alternative."""
        return cls('LeadingSize', value)

    @classmethod
    def termination_char(cls, value: str) -> ArgumentVariableStringChoice2:
        """Build the ``TerminationChar`` alternative."""
        return cls('TerminationChar', value)


@dataclass(slots=True)
class BaseDataChoice:
    """One alternative of a ``<choice>`` in :class:`BaseDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(cls,
                             value: BinaryDataEncodingType) -> BaseDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> BaseDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(cls,
                              value: IntegerDataEncodingType) -> BaseDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(cls,
                             value: StringDataEncodingType) -> BaseDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class BinaryAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`BinaryAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls, value: AlarmConditionsType) -> BinaryAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> BinaryAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class BinaryArgumentChoice:
    """One alternative of a ``<choice>`` in :class:`BinaryArgumentType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: ArgumentBinaryDataEncodingType) -> BinaryArgumentChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> BinaryArgumentChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> BinaryArgumentChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: ArgumentStringDataEncodingType) -> BinaryArgumentChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class BinaryContextAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`BinaryContextAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(
            cls, value: AlarmConditionsType) -> BinaryContextAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> BinaryContextAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class BinaryDataChoice:
    """One alternative of a ``<choice>`` in :class:`BinaryDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(cls,
                             value: BinaryDataEncodingType) -> BinaryDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> BinaryDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> BinaryDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(cls,
                             value: StringDataEncodingType) -> BinaryDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class BinaryParameterChoice:
    """One alternative of a ``<choice>`` in :class:`BinaryParameterType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> BinaryParameterChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> BinaryParameterChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> BinaryParameterChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> BinaryParameterChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class BooleanAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`BooleanAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls, value: AlarmConditionsType) -> BooleanAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> BooleanAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class BooleanArgumentChoice:
    """One alternative of a ``<choice>`` in :class:`BooleanArgumentType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> BooleanArgumentChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> BooleanArgumentChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> BooleanArgumentChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> BooleanArgumentChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class BooleanContextAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`BooleanContextAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(
            cls, value: AlarmConditionsType) -> BooleanContextAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> BooleanContextAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class BooleanDataChoice:
    """One alternative of a ``<choice>`` in :class:`BooleanDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> BooleanDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> BooleanDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> BooleanDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> BooleanDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class BooleanExpressionChoice:
    """One alternative of a ``<choice>`` in :class:`BooleanExpressionType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Condition'
    value: Any = field(default_factory=lambda: ComparisonCheckType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Condition", "ANDedConditions",
                                                 "ORedConditions"})

    @classmethod
    def condition(cls, value: ComparisonCheckType) -> BooleanExpressionChoice:
        """Build the ``Condition`` alternative."""
        return cls('Condition', value)

    @classmethod
    def an_ded_conditions(
            cls, value: ANDedConditionsType) -> BooleanExpressionChoice:
        """Build the ``ANDedConditions`` alternative."""
        return cls('ANDedConditions', value)

    @classmethod
    def o_red_conditions(cls,
                         value: ORedConditionsType) -> BooleanExpressionChoice:
        """Build the ``ORedConditions`` alternative."""
        return cls('ORedConditions', value)


@dataclass(slots=True)
class BooleanParameterChoice:
    """One alternative of a ``<choice>`` in :class:`BooleanParameterType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> BooleanParameterChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> BooleanParameterChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> BooleanParameterChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> BooleanParameterChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class CalibratorChoice:
    """One alternative of a ``<choice>`` in :class:`CalibratorType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'PolynomialCalibrator'
    value: Any = field(default_factory=lambda: PolynomialCalibratorType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"SplineCalibrator", "PolynomialCalibrator", "MathOperationCalibrator"})

    @classmethod
    def spline_calibrator(cls, value: SplineCalibratorType) -> CalibratorChoice:
        """Build the ``SplineCalibrator`` alternative."""
        return cls('SplineCalibrator', value)

    @classmethod
    def polynomial_calibrator(
            cls, value: PolynomialCalibratorType) -> CalibratorChoice:
        """Build the ``PolynomialCalibrator`` alternative."""
        return cls('PolynomialCalibrator', value)

    @classmethod
    def math_operation_calibrator(
            cls, value: MathOperationCalibratorType) -> CalibratorChoice:
        """Build the ``MathOperationCalibrator`` alternative."""
        return cls('MathOperationCalibrator', value)


@dataclass(slots=True)
class CommandContainerEntryListChoice:
    """One alternative of a ``<choice>`` in
    :class:`CommandContainerEntryListType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ParameterRefEntry'
    value: Any = field(default_factory=lambda: ArgumentParameterRefEntryType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ParameterRefEntry", "ParameterSegmentRefEntry", "ContainerRefEntry",
         "ContainerSegmentRefEntry", "StreamSegmentEntry",
         "IndirectParameterRefEntry", "ArrayParameterRefEntry",
         "ArgumentRefEntry", "ArrayArgumentRefEntry", "FixedValueEntry"})

    @classmethod
    def parameter_ref_entry(cls,
            value: ArgumentParameterRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ParameterRefEntry`` alternative."""
        return cls('ParameterRefEntry', value)

    @classmethod
    def parameter_segment_ref_entry(cls,
            value: ArgumentParameterSegmentRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ParameterSegmentRefEntry`` alternative."""
        return cls('ParameterSegmentRefEntry', value)

    @classmethod
    def container_ref_entry(cls,
            value: ArgumentContainerRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ContainerRefEntry`` alternative."""
        return cls('ContainerRefEntry', value)

    @classmethod
    def container_segment_ref_entry(cls,
            value: ArgumentContainerSegmentRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ContainerSegmentRefEntry`` alternative."""
        return cls('ContainerSegmentRefEntry', value)

    @classmethod
    def stream_segment_entry(cls,
            value: ArgumentStreamSegmentEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``StreamSegmentEntry`` alternative."""
        return cls('StreamSegmentEntry', value)

    @classmethod
    def indirect_parameter_ref_entry(cls,
            value: ArgumentIndirectParameterRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``IndirectParameterRefEntry`` alternative."""
        return cls('IndirectParameterRefEntry', value)

    @classmethod
    def array_parameter_ref_entry(cls,
            value: ArgumentArrayParameterRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ArrayParameterRefEntry`` alternative."""
        return cls('ArrayParameterRefEntry', value)

    @classmethod
    def argument_ref_entry(cls,
            value: ArgumentArgumentRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ArgumentRefEntry`` alternative."""
        return cls('ArgumentRefEntry', value)

    @classmethod
    def array_argument_ref_entry(cls,
            value: ArgumentArrayArgumentRefEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``ArrayArgumentRefEntry`` alternative."""
        return cls('ArrayArgumentRefEntry', value)

    @classmethod
    def fixed_value_entry(cls,
            value: ArgumentFixedValueEntryType
            ) -> CommandContainerEntryListChoice:
        """Build the ``FixedValueEntry`` alternative."""
        return cls('FixedValueEntry', value)


@dataclass(slots=True)
class CommandVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`CommandVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> CommandVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> CommandVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> CommandVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> CommandVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> CommandVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> CommandVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class CommandVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`CommandVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> CommandVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> CommandVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class ComparisonCheckChoice:
    """One alternative of a ``<choice>`` in :class:`ComparisonCheckType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ParameterInstanceRef'
    value: Any = field(default_factory=lambda: ParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"ParameterInstanceRef",
                                                 "Value"})

    @classmethod
    def parameter_instance_ref(
            cls, value: ParameterInstanceRefType) -> ComparisonCheckChoice:
        """Build the ``ParameterInstanceRef`` alternative."""
        return cls('ParameterInstanceRef', value)

    @classmethod
    def value_(cls, value: str) -> ComparisonCheckChoice:
        """Build the ``Value`` alternative."""
        return cls('Value', value)


@dataclass(slots=True)
class CompleteVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`CompleteVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> CompleteVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> CompleteVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> CompleteVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> CompleteVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> CompleteVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> CompleteVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class CompleteVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`CompleteVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> CompleteVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> CompleteVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class ContextMatchChoice:
    """One alternative of a ``<choice>`` in :class:`ContextMatchType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Comparison'
    value: Any = field(default_factory=lambda: ComparisonType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(cls, value: ComparisonType) -> ContextMatchChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(cls, value: ComparisonListType) -> ContextMatchChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(cls,
                           value: BooleanExpressionType) -> ContextMatchChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(cls, value: InputAlgorithmType) -> ContextMatchChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class DiscreteLookupChoice:
    """One alternative of a ``<choice>`` in :class:`DiscreteLookupType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Comparison'
    value: Any = field(default_factory=lambda: ComparisonType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(cls, value: ComparisonType) -> DiscreteLookupChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(cls, value: ComparisonListType) -> DiscreteLookupChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> DiscreteLookupChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> DiscreteLookupChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class EncodingChoice:
    """One alternative of a ``<choice>`` in :class:`EncodingType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(cls,
                             value: BinaryDataEncodingType) -> EncodingChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> EncodingChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(cls,
                              value: IntegerDataEncodingType) -> EncodingChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(cls,
                             value: StringDataEncodingType) -> EncodingChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class EntryListChoice:
    """One alternative of a ``<choice>`` in :class:`EntryListType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ParameterRefEntry'
    value: Any = field(default_factory=lambda: ParameterRefEntryType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ParameterRefEntry", "ParameterSegmentRefEntry", "ContainerRefEntry",
         "ContainerSegmentRefEntry", "StreamSegmentEntry",
         "IndirectParameterRefEntry", "ArrayParameterRefEntry"})

    @classmethod
    def parameter_ref_entry(cls,
                            value: ParameterRefEntryType) -> EntryListChoice:
        """Build the ``ParameterRefEntry`` alternative."""
        return cls('ParameterRefEntry', value)

    @classmethod
    def parameter_segment_ref_entry(
            cls, value: ParameterSegmentRefEntryType) -> EntryListChoice:
        """Build the ``ParameterSegmentRefEntry`` alternative."""
        return cls('ParameterSegmentRefEntry', value)

    @classmethod
    def container_ref_entry(cls,
                            value: ContainerRefEntryType) -> EntryListChoice:
        """Build the ``ContainerRefEntry`` alternative."""
        return cls('ContainerRefEntry', value)

    @classmethod
    def container_segment_ref_entry(
            cls, value: ContainerSegmentRefEntryType) -> EntryListChoice:
        """Build the ``ContainerSegmentRefEntry`` alternative."""
        return cls('ContainerSegmentRefEntry', value)

    @classmethod
    def stream_segment_entry(cls,
                             value: StreamSegmentEntryType) -> EntryListChoice:
        """Build the ``StreamSegmentEntry`` alternative."""
        return cls('StreamSegmentEntry', value)

    @classmethod
    def indirect_parameter_ref_entry(
            cls, value: IndirectParameterRefEntryType) -> EntryListChoice:
        """Build the ``IndirectParameterRefEntry`` alternative."""
        return cls('IndirectParameterRefEntry', value)

    @classmethod
    def array_parameter_ref_entry(
            cls, value: ArrayParameterRefEntryType) -> EntryListChoice:
        """Build the ``ArrayParameterRefEntry`` alternative."""
        return cls('ArrayParameterRefEntry', value)


@dataclass(slots=True)
class EnumeratedArgumentChoice:
    """One alternative of a ``<choice>`` in :class:`EnumeratedArgumentType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> EnumeratedArgumentChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> EnumeratedArgumentChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> EnumeratedArgumentChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> EnumeratedArgumentChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class EnumeratedDataChoice:
    """One alternative of a ``<choice>`` in :class:`EnumeratedDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> EnumeratedDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> EnumeratedDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> EnumeratedDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> EnumeratedDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class EnumeratedParameterChoice:
    """One alternative of a ``<choice>`` in :class:`EnumeratedParameterType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> EnumeratedParameterChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> EnumeratedParameterChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> EnumeratedParameterChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> EnumeratedParameterChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class EnumerationAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`EnumerationAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls,
                         value: AlarmConditionsType) -> EnumerationAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> EnumerationAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class EnumerationContextAlarmChoice:
    """One alternative of a ``<choice>`` in
    :class:`EnumerationContextAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(
            cls, value: AlarmConditionsType) -> EnumerationContextAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls,
                     value: CustomAlarmType) -> EnumerationContextAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class ErrorDetectCorrectChoice:
    """One alternative of a ``<choice>`` in :class:`ErrorDetectCorrectType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Checksum'
    value: Any = field(default_factory=lambda: ChecksumType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Checksum", "CRC", "XOR",
                                                 "Parity"})

    @classmethod
    def checksum(cls, value: ChecksumType) -> ErrorDetectCorrectChoice:
        """Build the ``Checksum`` alternative."""
        return cls('Checksum', value)

    @classmethod
    def crc(cls, value: CRCType) -> ErrorDetectCorrectChoice:
        """Build the ``CRC`` alternative."""
        return cls('CRC', value)

    @classmethod
    def xor(cls, value: XORType) -> ErrorDetectCorrectChoice:
        """Build the ``XOR`` alternative."""
        return cls('XOR', value)

    @classmethod
    def parity(cls, value: ParityType) -> ErrorDetectCorrectChoice:
        """Build the ``Parity`` alternative."""
        return cls('Parity', value)


@dataclass(slots=True)
class ExecutionVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`ExecutionVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> ExecutionVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> ExecutionVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> ExecutionVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> ExecutionVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> ExecutionVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> ExecutionVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class ExecutionVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`ExecutionVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> ExecutionVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> ExecutionVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class FailedVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`FailedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> FailedVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> FailedVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> FailedVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> FailedVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> FailedVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> FailedVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class FailedVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`FailedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> FailedVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> FailedVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class FixedFrameStreamChoice:
    """One alternative of a ``<choice>`` in :class:`FixedFrameStreamType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'ContainerRef', 'ServiceRef'})

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> FixedFrameStreamChoice:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def service_ref(cls, value: ServiceRefType) -> FixedFrameStreamChoice:
        """Build the ``ServiceRef`` alternative."""
        return cls('ServiceRef', value)


@dataclass(slots=True)
class FloatArgumentChoice:
    """One alternative of a ``<choice>`` in :class:`FloatArgumentType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: ArgumentBinaryDataEncodingType) -> FloatArgumentChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> FloatArgumentChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> FloatArgumentChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: ArgumentStringDataEncodingType) -> FloatArgumentChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class FloatDataChoice:
    """One alternative of a ``<choice>`` in :class:`FloatDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(cls,
                             value: BinaryDataEncodingType) -> FloatDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> FloatDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> FloatDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(cls,
                             value: StringDataEncodingType) -> FloatDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class FloatParameterChoice:
    """One alternative of a ``<choice>`` in :class:`FloatParameterType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> FloatParameterChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> FloatParameterChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> FloatParameterChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> FloatParameterChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class FrameStreamChoice:
    """One alternative of a ``<choice>`` in :class:`FrameStreamType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'ContainerRef', 'ServiceRef'})

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> FrameStreamChoice:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def service_ref(cls, value: ServiceRefType) -> FrameStreamChoice:
        """Build the ``ServiceRef`` alternative."""
        return cls('ServiceRef', value)


@dataclass(slots=True)
class InputSetChoice:
    """One alternative of a ``<choice>`` in :class:`InputSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'InputParameterInstanceRef'
    value: Any = field(default_factory=lambda: InputParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"InputParameterInstanceRef",
                                                 "Constant"})

    @classmethod
    def input_parameter_instance_ref(
            cls, value: InputParameterInstanceRefType) -> InputSetChoice:
        """Build the ``InputParameterInstanceRef`` alternative."""
        return cls('InputParameterInstanceRef', value)

    @classmethod
    def constant(cls, value: ConstantType) -> InputSetChoice:
        """Build the ``Constant`` alternative."""
        return cls('Constant', value)


@dataclass(slots=True)
class IntegerArgumentChoice:
    """One alternative of a ``<choice>`` in :class:`IntegerArgumentType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls,
            value: ArgumentBinaryDataEncodingType) -> IntegerArgumentChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> IntegerArgumentChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> IntegerArgumentChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls,
            value: ArgumentStringDataEncodingType) -> IntegerArgumentChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class IntegerDataChoice:
    """One alternative of a ``<choice>`` in :class:`IntegerDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> IntegerDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> IntegerDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> IntegerDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> IntegerDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class IntegerParameterChoice:
    """One alternative of a ``<choice>`` in :class:`IntegerParameterType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> IntegerParameterChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> IntegerParameterChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> IntegerParameterChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> IntegerParameterChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class IntegerValueChoice:
    """One alternative of a ``<choice>`` in :class:`IntegerValueType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FixedValue'
    value: Any = 0
    NAMES: ClassVar[frozenset[str]] = frozenset({"FixedValue", "DynamicValue",
                                                 "DiscreteLookupList"})

    @classmethod
    def fixed_value(cls, value: int) -> IntegerValueChoice:
        """Build the ``FixedValue`` alternative."""
        return cls('FixedValue', value)

    @classmethod
    def dynamic_value(cls, value: DynamicValueType) -> IntegerValueChoice:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)

    @classmethod
    def discrete_lookup_list(
            cls, value: DiscreteLookupListType) -> IntegerValueChoice:
        """Build the ``DiscreteLookupList`` alternative."""
        return cls('DiscreteLookupList', value)


@dataclass(slots=True)
class LocationInContainerInBitsChoice:
    """One alternative of a ``<choice>`` in
    :class:`LocationInContainerInBitsType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FixedValue'
    value: Any = 0
    NAMES: ClassVar[frozenset[str]] = frozenset({"FixedValue", "DynamicValue",
                                                 "DiscreteLookupList"})

    @classmethod
    def fixed_value(cls, value: int) -> LocationInContainerInBitsChoice:
        """Build the ``FixedValue`` alternative."""
        return cls('FixedValue', value)

    @classmethod
    def dynamic_value(
            cls, value: DynamicValueType) -> LocationInContainerInBitsChoice:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)

    @classmethod
    def discrete_lookup_list(
            cls,
            value: DiscreteLookupListType) -> LocationInContainerInBitsChoice:
        """Build the ``DiscreteLookupList`` alternative."""
        return cls('DiscreteLookupList', value)


@dataclass(slots=True)
class MatchCriteriaChoice:
    """One alternative of a ``<choice>`` in :class:`MatchCriteriaType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Comparison'
    value: Any = field(default_factory=lambda: ComparisonType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(cls, value: ComparisonType) -> MatchCriteriaChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(cls, value: ComparisonListType) -> MatchCriteriaChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(cls,
                           value: BooleanExpressionType) -> MatchCriteriaChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(cls, value: InputAlgorithmType) -> MatchCriteriaChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class MathOperationCalibratorChoice:
    """One alternative of a ``<choice>`` in
    :class:`MathOperationCalibratorType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ValueOperand'
    value: Any = ""
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ValueOperand", "ThisParameterOperand", "Operator",
         "ParameterInstanceRefOperand"})

    @classmethod
    def value_operand(cls, value: str) -> MathOperationCalibratorChoice:
        """Build the ``ValueOperand`` alternative."""
        return cls('ValueOperand', value)

    @classmethod
    def this_parameter_operand(cls,
                               value: str) -> MathOperationCalibratorChoice:
        """Build the ``ThisParameterOperand`` alternative."""
        return cls('ThisParameterOperand', value)

    @classmethod
    def operator(cls,
                 value: MathOperatorsType) -> MathOperationCalibratorChoice:
        """Build the ``Operator`` alternative."""
        return cls('Operator', value)

    @classmethod
    def parameter_instance_ref_operand(
            cls,
            value: ParameterInstanceRefType) -> MathOperationCalibratorChoice:
        """Build the ``ParameterInstanceRefOperand`` alternative."""
        return cls('ParameterInstanceRefOperand', value)


@dataclass(slots=True)
class MathOperationChoice:
    """One alternative of a ``<choice>`` in :class:`MathOperationType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ValueOperand'
    value: Any = ""
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ValueOperand", "ThisParameterOperand", "Operator",
         "ParameterInstanceRefOperand"})

    @classmethod
    def value_operand(cls, value: str) -> MathOperationChoice:
        """Build the ``ValueOperand`` alternative."""
        return cls('ValueOperand', value)

    @classmethod
    def this_parameter_operand(cls, value: str) -> MathOperationChoice:
        """Build the ``ThisParameterOperand`` alternative."""
        return cls('ThisParameterOperand', value)

    @classmethod
    def operator(cls, value: MathOperatorsType) -> MathOperationChoice:
        """Build the ``Operator`` alternative."""
        return cls('Operator', value)

    @classmethod
    def parameter_instance_ref_operand(
            cls, value: ParameterInstanceRefType) -> MathOperationChoice:
        """Build the ``ParameterInstanceRefOperand`` alternative."""
        return cls('ParameterInstanceRefOperand', value)


@dataclass(slots=True)
class MetaCommandSetChoice:
    """One alternative of a ``<choice>`` in :class:`MetaCommandSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'MetaCommand'
    value: Any = field(default_factory=lambda: MetaCommandType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"MetaCommand", "MetaCommandRef", "BlockMetaCommand"})

    @classmethod
    def meta_command(cls, value: MetaCommandType) -> MetaCommandSetChoice:
        """Build the ``MetaCommand`` alternative."""
        return cls('MetaCommand', value)

    @classmethod
    def meta_command_ref(cls, value: str) -> MetaCommandSetChoice:
        """Build the ``MetaCommandRef`` alternative."""
        return cls('MetaCommandRef', value)

    @classmethod
    def block_meta_command(cls,
                           value: BlockMetaCommandType) -> MetaCommandSetChoice:
        """Build the ``BlockMetaCommand`` alternative."""
        return cls('BlockMetaCommand', value)


@dataclass(slots=True)
class NumericAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`NumericAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls, value: AlarmConditionsType) -> NumericAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> NumericAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class NumericContextAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`NumericContextAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(
            cls, value: AlarmConditionsType) -> NumericContextAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> NumericContextAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class ORedConditionsChoice:
    """One alternative of a ``<choice>`` in :class:`ORedConditionsType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Condition'
    value: Any = field(default_factory=lambda: ComparisonCheckType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Condition",
                                                 "ANDedConditions"})

    @classmethod
    def condition(cls, value: ComparisonCheckType) -> ORedConditionsChoice:
        """Build the ``Condition`` alternative."""
        return cls('Condition', value)

    @classmethod
    def an_ded_conditions(cls,
                          value: ANDedConditionsType) -> ORedConditionsChoice:
        """Build the ``ANDedConditions`` alternative."""
        return cls('ANDedConditions', value)


@dataclass(slots=True)
class ParameterSetChoice:
    """One alternative of a ``<choice>`` in :class:`ParameterSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Parameter'
    value: Any = field(default_factory=lambda: ParameterType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'Parameter', 'ParameterRef'})

    @classmethod
    def parameter(cls, value: ParameterType) -> ParameterSetChoice:
        """Build the ``Parameter`` alternative."""
        return cls('Parameter', value)

    @classmethod
    def parameter_ref(cls, value: ParameterRefType) -> ParameterSetChoice:
        """Build the ``ParameterRef`` alternative."""
        return cls('ParameterRef', value)


@dataclass(slots=True)
class ParameterToSetChoice:
    """One alternative of a ``<choice>`` in :class:`ParameterToSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'NewValue'
    value: Any = ""
    NAMES: ClassVar[frozenset[str]] = frozenset({'Derivation', 'NewValue'})

    @classmethod
    def derivation(cls,
                   value: ArgumentMathOperationType) -> ParameterToSetChoice:
        """Build the ``Derivation`` alternative."""
        return cls('Derivation', value)

    @classmethod
    def new_value(cls, value: str) -> ParameterToSetChoice:
        """Build the ``NewValue`` alternative."""
        return cls('NewValue', value)


@dataclass(slots=True)
class ParameterTypeSetChoice:
    """One alternative of a ``<choice>`` in :class:`ParameterTypeSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'StringParameterType'
    value: Any = field(default_factory=lambda: StringParameterType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"StringParameterType", "EnumeratedParameterType",
         "IntegerParameterType", "BinaryParameterType", "FloatParameterType",
         "BooleanParameterType", "RelativeTimeParameterType",
         "AbsoluteTimeParameterType", "ArrayParameterType",
         "AggregateParameterType"})

    @classmethod
    def string_parameter_type(
            cls, value: StringParameterType) -> ParameterTypeSetChoice:
        """Build the ``StringParameterType`` alternative."""
        return cls('StringParameterType', value)

    @classmethod
    def enumerated_parameter_type(
            cls, value: EnumeratedParameterType) -> ParameterTypeSetChoice:
        """Build the ``EnumeratedParameterType`` alternative."""
        return cls('EnumeratedParameterType', value)

    @classmethod
    def integer_parameter_type(
            cls, value: IntegerParameterType) -> ParameterTypeSetChoice:
        """Build the ``IntegerParameterType`` alternative."""
        return cls('IntegerParameterType', value)

    @classmethod
    def binary_parameter_type(
            cls, value: BinaryParameterType) -> ParameterTypeSetChoice:
        """Build the ``BinaryParameterType`` alternative."""
        return cls('BinaryParameterType', value)

    @classmethod
    def float_parameter_type(
            cls, value: FloatParameterType) -> ParameterTypeSetChoice:
        """Build the ``FloatParameterType`` alternative."""
        return cls('FloatParameterType', value)

    @classmethod
    def boolean_parameter_type(
            cls, value: BooleanParameterType) -> ParameterTypeSetChoice:
        """Build the ``BooleanParameterType`` alternative."""
        return cls('BooleanParameterType', value)

    @classmethod
    def relative_time_parameter_type(
            cls, value: RelativeTimeParameterType) -> ParameterTypeSetChoice:
        """Build the ``RelativeTimeParameterType`` alternative."""
        return cls('RelativeTimeParameterType', value)

    @classmethod
    def absolute_time_parameter_type(
            cls, value: AbsoluteTimeParameterType) -> ParameterTypeSetChoice:
        """Build the ``AbsoluteTimeParameterType`` alternative."""
        return cls('AbsoluteTimeParameterType', value)

    @classmethod
    def array_parameter_type(
            cls, value: ArrayParameterType) -> ParameterTypeSetChoice:
        """Build the ``ArrayParameterType`` alternative."""
        return cls('ArrayParameterType', value)

    @classmethod
    def aggregate_parameter_type(
            cls, value: AggregateParameterType) -> ParameterTypeSetChoice:
        """Build the ``AggregateParameterType`` alternative."""
        return cls('AggregateParameterType', value)


@dataclass(slots=True)
class PercentCompleteChoice:
    """One alternative of a ``<choice>`` in :class:`PercentCompleteType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FixedValue'
    value: Any = 0.0
    NAMES: ClassVar[frozenset[str]] = frozenset({'FixedValue', 'DynamicValue'})

    @classmethod
    def fixed_value(cls, value: float) -> PercentCompleteChoice:
        """Build the ``FixedValue`` alternative."""
        return cls('FixedValue', value)

    @classmethod
    def dynamic_value(cls, value: DynamicValueType) -> PercentCompleteChoice:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)


@dataclass(slots=True)
class QueuedVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`QueuedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> QueuedVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> QueuedVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> QueuedVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> QueuedVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> QueuedVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> QueuedVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class QueuedVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`QueuedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> QueuedVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> QueuedVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class ReceivedVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`ReceivedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> ReceivedVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls, value: ContainerRefType) -> ReceivedVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls, value: ParameterValueChangeType) -> ReceivedVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(cls,
                         value: InputAlgorithmType) -> ReceivedVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> ReceivedVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> ReceivedVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class ReceivedVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`ReceivedVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls, value: CheckWindowType) -> ReceivedVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls, value: CheckWindowAlgorithmsType) -> ReceivedVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class ReferenceTimeChoice:
    """One alternative of a ``<choice>`` in :class:`ReferenceTimeType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'OffsetFrom'
    value: Any = field(default_factory=lambda: ParameterInstanceRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'OffsetFrom', 'Epoch'})

    @classmethod
    def offset_from(cls,
                    value: ParameterInstanceRefType) -> ReferenceTimeChoice:
        """Build the ``OffsetFrom`` alternative."""
        return cls('OffsetFrom', value)

    @classmethod
    def epoch(cls, value: str) -> ReferenceTimeChoice:
        """Build the ``Epoch`` alternative."""
        return cls('Epoch', value)


@dataclass(slots=True)
class RestrictionCriteriaChoice:
    """One alternative of a ``<choice>`` in :class:`RestrictionCriteriaType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Comparison'
    value: Any = field(default_factory=lambda: ComparisonType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(cls, value: ComparisonType) -> RestrictionCriteriaChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(cls,
                        value: ComparisonListType) -> RestrictionCriteriaChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> RestrictionCriteriaChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(
            cls, value: InputAlgorithmType) -> RestrictionCriteriaChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class SentFromRangeVerifierChoice1:
    """One alternative of a ``<choice>`` in :class:`SentFromRangeVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(
            cls, value: ComparisonListType) -> SentFromRangeVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(cls,
                      value: ContainerRefType) -> SentFromRangeVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(
            cls,
            value: ParameterValueChangeType) -> SentFromRangeVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(
            cls, value: InputAlgorithmType) -> SentFromRangeVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> SentFromRangeVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls, value: ComparisonType) -> SentFromRangeVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class SentFromRangeVerifierChoice2:
    """One alternative of a ``<choice>`` in :class:`SentFromRangeVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(cls,
                     value: CheckWindowType) -> SentFromRangeVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(
            cls,
            value: CheckWindowAlgorithmsType) -> SentFromRangeVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class ServiceChoice:
    """One alternative of a ``<choice>`` in :class:`ServiceType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'MessageRefSet'
    value: Any = field(default_factory=lambda: MessageRefSetType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"MessageRefSet",
                                                 "ContainerRefSet"})

    @classmethod
    def message_ref_set(cls, value: MessageRefSetType) -> ServiceChoice:
        """Build the ``MessageRefSet`` alternative."""
        return cls('MessageRefSet', value)

    @classmethod
    def container_ref_set(cls, value: ContainerRefSetType) -> ServiceChoice:
        """Build the ``ContainerRefSet`` alternative."""
        return cls('ContainerRefSet', value)


@dataclass(slots=True)
class StreamSetChoice:
    """One alternative of a ``<choice>`` in :class:`StreamSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CustomStream'
    value: Any = field(default_factory=lambda: CustomStreamType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"FixedFrameStream", "VariableFrameStream", "CustomStream"})

    @classmethod
    def fixed_frame_stream(cls, value: FixedFrameStreamType) -> StreamSetChoice:
        """Build the ``FixedFrameStream`` alternative."""
        return cls('FixedFrameStream', value)

    @classmethod
    def variable_frame_stream(
            cls, value: VariableFrameStreamType) -> StreamSetChoice:
        """Build the ``VariableFrameStream`` alternative."""
        return cls('VariableFrameStream', value)

    @classmethod
    def custom_stream(cls, value: CustomStreamType) -> StreamSetChoice:
        """Build the ``CustomStream`` alternative."""
        return cls('CustomStream', value)


@dataclass(slots=True)
class StringAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`StringAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls, value: AlarmConditionsType) -> StringAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> StringAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class StringArgumentChoice:
    """One alternative of a ``<choice>`` in :class:`StringArgumentType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: ArgumentBinaryDataEncodingType) -> StringArgumentChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> StringArgumentChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> StringArgumentChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: ArgumentStringDataEncodingType) -> StringArgumentChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class StringContextAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`StringContextAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(
            cls, value: AlarmConditionsType) -> StringContextAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> StringContextAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class StringDataChoice:
    """One alternative of a ``<choice>`` in :class:`StringDataType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(cls,
                             value: BinaryDataEncodingType) -> StringDataChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(cls,
                            value: FloatDataEncodingType) -> StringDataChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> StringDataChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(cls,
                             value: StringDataEncodingType) -> StringDataChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class StringDataEncodingChoice:
    """One alternative of a ``<choice>`` in :class:`StringDataEncodingType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'SizeInBits'
    value: Any = field(default_factory=lambda: SizeInBitsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'SizeInBits', 'Variable'})

    @classmethod
    def size_in_bits(cls, value: SizeInBitsType) -> StringDataEncodingChoice:
        """Build the ``SizeInBits`` alternative."""
        return cls('SizeInBits', value)

    @classmethod
    def variable(cls, value: VariableStringType) -> StringDataEncodingChoice:
        """Build the ``Variable`` alternative."""
        return cls('Variable', value)


@dataclass(slots=True)
class StringParameterChoice:
    """One alternative of a ``<choice>`` in :class:`StringParameterType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'FloatDataEncoding'
    value: Any = field(default_factory=lambda: FloatDataEncodingType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"BinaryDataEncoding", "FloatDataEncoding", "IntegerDataEncoding",
         "StringDataEncoding"})

    @classmethod
    def binary_data_encoding(
            cls, value: BinaryDataEncodingType) -> StringParameterChoice:
        """Build the ``BinaryDataEncoding`` alternative."""
        return cls('BinaryDataEncoding', value)

    @classmethod
    def float_data_encoding(
            cls, value: FloatDataEncodingType) -> StringParameterChoice:
        """Build the ``FloatDataEncoding`` alternative."""
        return cls('FloatDataEncoding', value)

    @classmethod
    def integer_data_encoding(
            cls, value: IntegerDataEncodingType) -> StringParameterChoice:
        """Build the ``IntegerDataEncoding`` alternative."""
        return cls('IntegerDataEncoding', value)

    @classmethod
    def string_data_encoding(
            cls, value: StringDataEncodingType) -> StringParameterChoice:
        """Build the ``StringDataEncoding`` alternative."""
        return cls('StringDataEncoding', value)


@dataclass(slots=True)
class TimeAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`TimeAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls, value: AlarmConditionsType) -> TimeAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> TimeAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class TimeContextAlarmChoice:
    """One alternative of a ``<choice>`` in :class:`TimeContextAlarmType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'AlarmConditions'
    value: Any = field(default_factory=lambda: AlarmConditionsType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"AlarmConditions",
                                                 "CustomAlarm"})

    @classmethod
    def alarm_conditions(cls,
                         value: AlarmConditionsType) -> TimeContextAlarmChoice:
        """Build the ``AlarmConditions`` alternative."""
        return cls('AlarmConditions', value)

    @classmethod
    def custom_alarm(cls, value: CustomAlarmType) -> TimeContextAlarmChoice:
        """Build the ``CustomAlarm`` alternative."""
        return cls('CustomAlarm', value)


@dataclass(slots=True)
class TransferredToRangeVerifierChoice1:
    """One alternative of a ``<choice>`` in
    :class:`TransferredToRangeVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ComparisonList", "ContainerRef", "ParameterValueChange",
         "CustomAlgorithm", "BooleanExpression", "Comparison"})

    @classmethod
    def comparison_list(
            cls,
            value: ComparisonListType) -> TransferredToRangeVerifierChoice1:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def container_ref(
            cls, value: ContainerRefType) -> TransferredToRangeVerifierChoice1:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def parameter_value_change(cls,
            value: ParameterValueChangeType
            ) -> TransferredToRangeVerifierChoice1:
        """Build the ``ParameterValueChange`` alternative."""
        return cls('ParameterValueChange', value)

    @classmethod
    def custom_algorithm(
            cls,
            value: InputAlgorithmType) -> TransferredToRangeVerifierChoice1:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)

    @classmethod
    def boolean_expression(
            cls,
            value: BooleanExpressionType) -> TransferredToRangeVerifierChoice1:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def comparison(cls,
                   value: ComparisonType) -> TransferredToRangeVerifierChoice1:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)


@dataclass(slots=True)
class TransferredToRangeVerifierChoice2:
    """One alternative of a ``<choice>`` in
    :class:`TransferredToRangeVerifierType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'CheckWindow'
    value: Any = field(default_factory=lambda: CheckWindowType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"CheckWindow",
                                                 "CheckWindowAlgorithms"})

    @classmethod
    def check_window(
            cls, value: CheckWindowType) -> TransferredToRangeVerifierChoice2:
        """Build the ``CheckWindow`` alternative."""
        return cls('CheckWindow', value)

    @classmethod
    def check_window_algorithms(cls,
            value: CheckWindowAlgorithmsType
            ) -> TransferredToRangeVerifierChoice2:
        """Build the ``CheckWindowAlgorithms`` alternative."""
        return cls('CheckWindowAlgorithms', value)


@dataclass(slots=True)
class TransmissionConstraintChoice:
    """One alternative of a ``<choice>`` in :class:`TransmissionConstraintType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'Comparison'
    value: Any = field(default_factory=lambda: ComparisonType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"Comparison", "ComparisonList",
                                                 "BooleanExpression",
                                                 "CustomAlgorithm"})

    @classmethod
    def comparison(cls, value: ComparisonType) -> TransmissionConstraintChoice:
        """Build the ``Comparison`` alternative."""
        return cls('Comparison', value)

    @classmethod
    def comparison_list(
            cls, value: ComparisonListType) -> TransmissionConstraintChoice:
        """Build the ``ComparisonList`` alternative."""
        return cls('ComparisonList', value)

    @classmethod
    def boolean_expression(
            cls, value: BooleanExpressionType) -> TransmissionConstraintChoice:
        """Build the ``BooleanExpression`` alternative."""
        return cls('BooleanExpression', value)

    @classmethod
    def custom_algorithm(
            cls, value: InputAlgorithmType) -> TransmissionConstraintChoice:
        """Build the ``CustomAlgorithm`` alternative."""
        return cls('CustomAlgorithm', value)


@dataclass(slots=True)
class TriggerSetChoice:
    """One alternative of a ``<choice>`` in :class:`TriggerSetType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'OnParameterUpdateTrigger'
    value: Any = field(default_factory=lambda: OnParameterUpdateTriggerType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"OnParameterUpdateTrigger",
                                                 "OnContainerUpdateTrigger",
                                                 "OnPeriodicRateTrigger"})

    @classmethod
    def on_parameter_update_trigger(
            cls, value: OnParameterUpdateTriggerType) -> TriggerSetChoice:
        """Build the ``OnParameterUpdateTrigger`` alternative."""
        return cls('OnParameterUpdateTrigger', value)

    @classmethod
    def on_container_update_trigger(
            cls, value: OnContainerUpdateTriggerType) -> TriggerSetChoice:
        """Build the ``OnContainerUpdateTrigger`` alternative."""
        return cls('OnContainerUpdateTrigger', value)

    @classmethod
    def on_periodic_rate_trigger(
            cls, value: OnPeriodicRateTriggerType) -> TriggerSetChoice:
        """Build the ``OnPeriodicRateTrigger`` alternative."""
        return cls('OnPeriodicRateTrigger', value)


@dataclass(slots=True)
class TriggeredMathOperationChoice:
    """One alternative of a ``<choice>`` in :class:`TriggeredMathOperationType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ValueOperand'
    value: Any = ""
    NAMES: ClassVar[frozenset[str]] = frozenset(
        {"ValueOperand", "ThisParameterOperand", "Operator",
         "ParameterInstanceRefOperand"})

    @classmethod
    def value_operand(cls, value: str) -> TriggeredMathOperationChoice:
        """Build the ``ValueOperand`` alternative."""
        return cls('ValueOperand', value)

    @classmethod
    def this_parameter_operand(cls, value: str) -> TriggeredMathOperationChoice:
        """Build the ``ThisParameterOperand`` alternative."""
        return cls('ThisParameterOperand', value)

    @classmethod
    def operator(cls, value: MathOperatorsType) -> TriggeredMathOperationChoice:
        """Build the ``Operator`` alternative."""
        return cls('Operator', value)

    @classmethod
    def parameter_instance_ref_operand(
            cls,
            value: ParameterInstanceRefType) -> TriggeredMathOperationChoice:
        """Build the ``ParameterInstanceRefOperand`` alternative."""
        return cls('ParameterInstanceRefOperand', value)


@dataclass(slots=True)
class VariableFrameStreamChoice:
    """One alternative of a ``<choice>`` in :class:`VariableFrameStreamType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'ContainerRef'
    value: Any = field(default_factory=lambda: ContainerRefType())
    NAMES: ClassVar[frozenset[str]] = frozenset({'ContainerRef', 'ServiceRef'})

    @classmethod
    def container_ref(cls,
                      value: ContainerRefType) -> VariableFrameStreamChoice:
        """Build the ``ContainerRef`` alternative."""
        return cls('ContainerRef', value)

    @classmethod
    def service_ref(cls, value: ServiceRefType) -> VariableFrameStreamChoice:
        """Build the ``ServiceRef`` alternative."""
        return cls('ServiceRef', value)


@dataclass(slots=True)
class VariableStringChoice1:
    """One alternative of a ``<choice>`` in :class:`VariableStringType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'DynamicValue'
    value: Any = field(default_factory=lambda: DynamicValueType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"DynamicValue",
                                                 "DiscreteLookupList"})

    @classmethod
    def dynamic_value(cls, value: DynamicValueType) -> VariableStringChoice1:
        """Build the ``DynamicValue`` alternative."""
        return cls('DynamicValue', value)

    @classmethod
    def discrete_lookup_list(
            cls, value: DiscreteLookupListType) -> VariableStringChoice1:
        """Build the ``DiscreteLookupList`` alternative."""
        return cls('DiscreteLookupList', value)


@dataclass(slots=True)
class VariableStringChoice2:
    """One alternative of a ``<choice>`` in :class:`VariableStringType`.

    ``name`` is the element name of the alternative and ``value`` its value;
    use the class-method constructors to build one.
    """

    name: str = 'LeadingSize'
    value: Any = field(default_factory=lambda: LeadingSizeType())
    NAMES: ClassVar[frozenset[str]] = frozenset({"LeadingSize",
                                                 "TerminationChar"})

    @classmethod
    def leading_size(cls, value: LeadingSizeType) -> VariableStringChoice2:
        """Build the ``LeadingSize`` alternative."""
        return cls('LeadingSize', value)

    @classmethod
    def termination_char(cls, value: str) -> VariableStringChoice2:
        """Build the ``TerminationChar`` alternative."""
        return cls('TerminationChar', value)

# -- deserialization ----------------------------------------------------------


_A_ANDedConditionsType = frozenset({})


def _r_ANDedConditionsType(n: X.Node) -> ANDedConditionsType:
    X.check_attrs(n, _A_ANDedConditionsType)
    v = ANDedConditionsType.__new__(ANDedConditionsType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ANDedConditionsChoice.NAMES:
        items.append(_r_ANDedConditionsChoice(c))
    if len(items) < 2:
        raise c.too_few('Condition|ORedConditions', 2, len(items))
    v.items = items
    c.end()
    return v


_A_AbsoluteTimeArgumentType = frozenset({"shortDescription", "name", "baseType",
                                         "initialValue"})


def _r_AbsoluteTimeArgumentType(n: X.Node) -> AbsoluteTimeArgumentType:
    X.check_attrs(n, _A_AbsoluteTimeArgumentType)
    v = AbsoluteTimeArgumentType.__new__(AbsoluteTimeArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_AbsoluteTimeDataType = frozenset({"shortDescription", "name", "baseType",
                                     "initialValue"})


def _r_AbsoluteTimeDataType(n: X.Node) -> AbsoluteTimeDataType:
    X.check_attrs(n, _A_AbsoluteTimeDataType)
    v = AbsoluteTimeDataType.__new__(AbsoluteTimeDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_AbsoluteTimeParameterType = frozenset({"shortDescription", "name",
                                          "baseType", "initialValue"})


def _r_AbsoluteTimeParameterType(n: X.Node) -> AbsoluteTimeParameterType:
    X.check_attrs(n, _A_AbsoluteTimeParameterType)
    v = AbsoluteTimeParameterType.__new__(AbsoluteTimeParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_AcceptedVerifierType = frozenset({'shortDescription', 'name'})


def _r_AcceptedVerifierType(n: X.Node) -> AcceptedVerifierType:
    X.check_attrs(n, _A_AcceptedVerifierType)
    v = AcceptedVerifierType.__new__(AcceptedVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_AcceptedVerifierChoice1(c)
    v.choice2 = _r_AcceptedVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_AggregateArgumentType = frozenset({"shortDescription", "name",
                                      "initialValue"})


def _r_AggregateArgumentType(n: X.Node) -> AggregateArgumentType:
    X.check_attrs(n, _A_AggregateArgumentType)
    v = AggregateArgumentType.__new__(AggregateArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'MemberList':
        raise c.expected('MemberList')
    v.member_list = _r_MemberListType(c.take())
    c.end()
    return v


_A_AggregateDataType = frozenset({'shortDescription', 'name', 'initialValue'})


def _r_AggregateDataType(n: X.Node) -> AggregateDataType:
    X.check_attrs(n, _A_AggregateDataType)
    v = AggregateDataType.__new__(AggregateDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'MemberList':
        raise c.expected('MemberList')
    v.member_list = _r_MemberListType(c.take())
    c.end()
    return v


_A_AggregateParameterType = frozenset({"shortDescription", "name",
                                       "initialValue"})


def _r_AggregateParameterType(n: X.Node) -> AggregateParameterType:
    X.check_attrs(n, _A_AggregateParameterType)
    v = AggregateParameterType.__new__(AggregateParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'MemberList':
        raise c.expected('MemberList')
    v.member_list = _r_MemberListType(c.take())
    c.end()
    return v


_A_AlarmConditionsType = frozenset({})


def _r_AlarmConditionsType(n: X.Node) -> AlarmConditionsType:
    X.check_attrs(n, _A_AlarmConditionsType)
    v = AlarmConditionsType.__new__(AlarmConditionsType)
    c = X.Cursor(n)
    v.watch_alarm = (_r_MatchCriteriaType(c.take()) if c.peek() == "WatchAlarm"
                     else None)
    v.warning_alarm = (_r_MatchCriteriaType(c.take())
                       if c.peek() == "WarningAlarm" else None)
    v.distress_alarm = (_r_MatchCriteriaType(c.take())
                        if c.peek() == "DistressAlarm" else None)
    v.critical_alarm = (_r_MatchCriteriaType(c.take())
                        if c.peek() == "CriticalAlarm" else None)
    v.severe_alarm = (_r_MatchCriteriaType(c.take())
                      if c.peek() == "SevereAlarm" else None)
    c.end()
    return v


_A_AlarmMultiRangesType = frozenset({'name', 'shortDescription'})


def _r_AlarmMultiRangesType(n: X.Node) -> AlarmMultiRangesType:
    X.check_attrs(n, _A_AlarmMultiRangesType)
    v = AlarmMultiRangesType.__new__(AlarmMultiRangesType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() == 'Range':
        items.append(_r_MultiRangeType(c.take()))
    if len(items) < 1:
        raise c.too_few('Range', 1, len(items))
    v.range = items
    c.end()
    return v


_A_AlarmRangesType = frozenset({'name', 'shortDescription', 'rangeForm'})


def _r_AlarmRangesType(n: X.Node) -> AlarmRangesType:
    X.check_attrs(n, _A_AlarmRangesType)
    v = AlarmRangesType.__new__(AlarmRangesType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.range_form = X.opt_attr(n, 'rangeForm', _p_RangeFormType)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.watch_range = (_r_FloatRangeType(c.take()) if c.peek() == "WatchRange"
                     else None)
    v.warning_range = (_r_FloatRangeType(c.take()) if c.peek() == "WarningRange"
                       else None)
    v.distress_range = (_r_FloatRangeType(c.take())
                        if c.peek() == "DistressRange" else None)
    v.critical_range = (_r_FloatRangeType(c.take())
                        if c.peek() == "CriticalRange" else None)
    v.severe_range = (_r_FloatRangeType(c.take()) if c.peek() == "SevereRange"
                      else None)
    c.end()
    return v


_A_AlarmType = frozenset({"name", "shortDescription", "minViolations",
                          "minConformance", "disabled"})


def _r_AlarmType(n: X.Node) -> AlarmType:
    X.check_attrs(n, _A_AlarmType)
    v = AlarmType.__new__(AlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = _r_AlarmChoice(c) if c.peek() in AlarmChoice.NAMES else None
    c.end()
    return v


_A_AlgorithmSetType = frozenset({})


def _r_AlgorithmSetType(n: X.Node) -> AlgorithmSetType:
    X.check_attrs(n, _A_AlgorithmSetType)
    v = AlgorithmSetType.__new__(AlgorithmSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in AlgorithmSetChoice.NAMES:
        items.append(_r_AlgorithmSetChoice(c))
    if len(items) < 1:
        raise c.too_few('CustomAlgorithm|MathAlgorithm', 1, len(items))
    v.items = items
    c.end()
    return v


_A_AlgorithmTextType = frozenset({'language'})


def _r_AlgorithmTextType(n: X.Node) -> AlgorithmTextType:
    X.check_attrs(n, _A_AlgorithmTextType)
    v = AlgorithmTextType.__new__(AlgorithmTextType)
    v.language = X.opt_attr(n, 'language', X.p_str)
    v.value = X.value(n, X.text_of(n), X.p_str)
    return v


_A_AliasSetType = frozenset({})


def _r_AliasSetType(n: X.Node) -> AliasSetType:
    X.check_attrs(n, _A_AliasSetType)
    v = AliasSetType.__new__(AliasSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Alias':
        items.append(_r_AliasType(c.take()))
    if len(items) < 1:
        raise c.too_few('Alias', 1, len(items))
    v.alias = items
    c.end()
    return v


_A_AliasType = frozenset({'nameSpace', 'alias'})


def _r_AliasType(n: X.Node) -> AliasType:
    X.check_attrs(n, _A_AliasType)
    v = AliasType.__new__(AliasType)
    v.name_space = X.req_attr(n, 'nameSpace', X.p_str)
    v.alias = X.req_attr(n, 'alias', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_AncillaryDataSetType = frozenset({})


def _r_AncillaryDataSetType(n: X.Node) -> AncillaryDataSetType:
    X.check_attrs(n, _A_AncillaryDataSetType)
    v = AncillaryDataSetType.__new__(AncillaryDataSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'AncillaryData':
        items.append(_r_AncillaryDataType(c.take()))
    if len(items) < 1:
        raise c.too_few('AncillaryData', 1, len(items))
    v.ancillary_data = items
    c.end()
    return v


_A_AncillaryDataType = frozenset({'name', 'mimeType', 'href'})


def _r_AncillaryDataType(n: X.Node) -> AncillaryDataType:
    X.check_attrs(n, _A_AncillaryDataType)
    v = AncillaryDataType.__new__(AncillaryDataType)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.mime_type = X.opt_attr(n, 'mimeType', X.p_str)
    v.href = X.opt_attr(n, 'href', X.p_str)
    v.value = X.value(n, X.text_of(n), X.p_str)
    return v


_A_ArgumentANDedConditionsType = frozenset({})


def _r_ArgumentANDedConditionsType(n: X.Node) -> ArgumentANDedConditionsType:
    X.check_attrs(n, _A_ArgumentANDedConditionsType)
    v = ArgumentANDedConditionsType.__new__(ArgumentANDedConditionsType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ArgumentANDedConditionsChoice.NAMES:
        items.append(_r_ArgumentANDedConditionsChoice(c))
    if len(items) < 2:
        raise c.too_few('Condition|ORedConditions', 2, len(items))
    v.items = items
    c.end()
    return v


_A_ArgumentAbsoluteTimeDataType = frozenset({"shortDescription", "name",
                                             "baseType", "initialValue"})


def _r_ArgumentAbsoluteTimeDataType(n: X.Node) -> ArgumentAbsoluteTimeDataType:
    X.check_attrs(n, _A_ArgumentAbsoluteTimeDataType)
    v = ArgumentAbsoluteTimeDataType.__new__(ArgumentAbsoluteTimeDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_ArgumentArgumentRefEntryType = frozenset({'shortDescription', 'argumentRef'})


def _r_ArgumentArgumentRefEntryType(n: X.Node) -> ArgumentArgumentRefEntryType:
    X.check_attrs(n, _A_ArgumentArgumentRefEntryType)
    v = ArgumentArgumentRefEntryType.__new__(ArgumentArgumentRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.argument_ref = X.req_attr(n, 'argumentRef', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentArrayArgumentRefEntryType = frozenset(
    {"shortDescription", "argumentRef", "lastEntryForThisArrayInstance"})


def _r_ArgumentArrayArgumentRefEntryType(
        n: X.Node) -> ArgumentArrayArgumentRefEntryType:
    X.check_attrs(n, _A_ArgumentArrayArgumentRefEntryType)
    v = ArgumentArrayArgumentRefEntryType.__new__(
        ArgumentArrayArgumentRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.argument_ref = X.req_attr(n, 'argumentRef', X.p_str)
    v.last_entry_for_this_array_instance = X.opt_attr(
        n, "lastEntryForThisArrayInstance", X.p_bool)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.dimension_list = (_r_ArgumentDimensionListType(c.take())
                        if c.peek() == "DimensionList" else None)
    c.end()
    return v


_A_ArgumentArrayParameterRefEntryType = frozenset(
    {"shortDescription", "parameterRef", "lastEntryForThisArrayInstance"})


def _r_ArgumentArrayParameterRefEntryType(
        n: X.Node) -> ArgumentArrayParameterRefEntryType:
    X.check_attrs(n, _A_ArgumentArrayParameterRefEntryType)
    v = ArgumentArrayParameterRefEntryType.__new__(
        ArgumentArrayParameterRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.last_entry_for_this_array_instance = X.opt_attr(
        n, "lastEntryForThisArrayInstance", X.p_bool)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.dimension_list = (_r_DimensionListType(c.take())
                        if c.peek() == "DimensionList" else None)
    c.end()
    return v


_A_ArgumentAssignmentListType = frozenset({})


def _r_ArgumentAssignmentListType(n: X.Node) -> ArgumentAssignmentListType:
    X.check_attrs(n, _A_ArgumentAssignmentListType)
    v = ArgumentAssignmentListType.__new__(ArgumentAssignmentListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ArgumentAssignment':
        items.append(_r_ArgumentAssignmentType(c.take()))
    if len(items) < 1:
        raise c.too_few('ArgumentAssignment', 1, len(items))
    v.argument_assignment = items
    c.end()
    return v


_A_ArgumentAssignmentType = frozenset({'argumentName', 'argumentValue'})


def _r_ArgumentAssignmentType(n: X.Node) -> ArgumentAssignmentType:
    X.check_attrs(n, _A_ArgumentAssignmentType)
    v = ArgumentAssignmentType.__new__(ArgumentAssignmentType)
    v.argument_name = X.req_attr(n, 'argumentName', X.p_str)
    v.argument_value = X.req_attr(n, 'argumentValue', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_ArgumentBaseDataType = frozenset({'shortDescription', 'name', 'baseType'})


def _r_ArgumentBaseDataType(n: X.Node) -> ArgumentBaseDataType:
    X.check_attrs(n, _A_ArgumentBaseDataType)
    v = ArgumentBaseDataType.__new__(ArgumentBaseDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentBaseDataChoice(c)
                if c.peek() in ArgumentBaseDataChoice.NAMES else None)
    c.end()
    return v


_A_ArgumentBaseTimeDataType = frozenset({"shortDescription", "name",
                                         "baseType"})


def _r_ArgumentBaseTimeDataType(n: X.Node) -> ArgumentBaseTimeDataType:
    X.check_attrs(n, _A_ArgumentBaseTimeDataType)
    v = ArgumentBaseTimeDataType.__new__(ArgumentBaseTimeDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_ArgumentBinaryDataEncodingType = frozenset({'bitOrder', 'byteOrder'})


def _r_ArgumentBinaryDataEncodingType(
        n: X.Node) -> ArgumentBinaryDataEncodingType:
    X.check_attrs(n, _A_ArgumentBinaryDataEncodingType)
    v = ArgumentBinaryDataEncodingType.__new__(ArgumentBinaryDataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    if c.peek() != 'SizeInBits':
        raise c.expected('SizeInBits')
    v.size_in_bits = _r_ArgumentIntegerValueType(c.take())
    v.from_binary_transform_algorithm = _r_ArgumentInputAlgorithmType(
        c.take()) if c.peek() == 'FromBinaryTransformAlgorithm' else None
    v.to_binary_transform_algorithm = _r_ArgumentInputAlgorithmType(
        c.take()) if c.peek() == 'ToBinaryTransformAlgorithm' else None
    c.end()
    return v


_A_ArgumentBinaryDataType = frozenset({"shortDescription", "name", "baseType",
                                       "initialValue"})


def _r_ArgumentBinaryDataType(n: X.Node) -> ArgumentBinaryDataType:
    X.check_attrs(n, _A_ArgumentBinaryDataType)
    v = ArgumentBinaryDataType.__new__(ArgumentBinaryDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentBinaryDataChoice(c)
                if c.peek() in ArgumentBinaryDataChoice.NAMES else None)
    c.end()
    return v


_A_ArgumentBooleanDataType = frozenset({"shortDescription", "name", "baseType",
                                        "initialValue", "oneStringValue",
                                        "zeroStringValue"})


def _r_ArgumentBooleanDataType(n: X.Node) -> ArgumentBooleanDataType:
    X.check_attrs(n, _A_ArgumentBooleanDataType)
    v = ArgumentBooleanDataType.__new__(ArgumentBooleanDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.one_string_value = X.opt_attr(n, 'oneStringValue', X.p_str)
    v.zero_string_value = X.opt_attr(n, 'zeroStringValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentBooleanDataChoice(c)
                if c.peek() in ArgumentBooleanDataChoice.NAMES else None)
    c.end()
    return v


_A_ArgumentBooleanExpressionType = frozenset({})


def _r_ArgumentBooleanExpressionType(
        n: X.Node) -> ArgumentBooleanExpressionType:
    X.check_attrs(n, _A_ArgumentBooleanExpressionType)
    v = ArgumentBooleanExpressionType.__new__(ArgumentBooleanExpressionType)
    c = X.Cursor(n)
    v.choice = _r_ArgumentBooleanExpressionChoice(c)
    c.end()
    return v


_A_ArgumentComparisonCheckType = frozenset({})


def _r_ArgumentComparisonCheckType(n: X.Node) -> ArgumentComparisonCheckType:
    X.check_attrs(n, _A_ArgumentComparisonCheckType)
    v = ArgumentComparisonCheckType.__new__(ArgumentComparisonCheckType)
    c = X.Cursor(n)
    v.choice1 = _r_ArgumentComparisonCheckChoice1(c)
    if c.peek() != 'ComparisonOperator':
        raise c.expected('ComparisonOperator')
    v.comparison_operator = X.leaf(c.take(), _p_ComparisonOperatorsType)
    v.choice2 = _r_ArgumentComparisonCheckChoice2(c)
    c.end()
    return v


_A_ArgumentComparisonListType = frozenset({})


def _r_ArgumentComparisonListType(n: X.Node) -> ArgumentComparisonListType:
    X.check_attrs(n, _A_ArgumentComparisonListType)
    v = ArgumentComparisonListType.__new__(ArgumentComparisonListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Comparison':
        items.append(_r_ArgumentComparisonType(c.take()))
    if len(items) < 1:
        raise c.too_few('Comparison', 1, len(items))
    v.comparison = items
    c.end()
    return v


_A_ArgumentComparisonType = frozenset({'comparisonOperator', 'value'})


def _r_ArgumentComparisonType(n: X.Node) -> ArgumentComparisonType:
    X.check_attrs(n, _A_ArgumentComparisonType)
    v = ArgumentComparisonType.__new__(ArgumentComparisonType)
    v.comparison_operator = X.opt_attr(n, "comparisonOperator",
                                       _p_ComparisonOperatorsType)
    v.value = X.req_attr(n, 'value', X.p_str)
    c = X.Cursor(n)
    v.choice = _r_ArgumentComparisonChoice(c)
    c.end()
    return v


_A_ArgumentContainerRefEntryType = frozenset({"shortDescription",
                                              "containerRef"})


def _r_ArgumentContainerRefEntryType(
        n: X.Node) -> ArgumentContainerRefEntryType:
    X.check_attrs(n, _A_ArgumentContainerRefEntryType)
    v = ArgumentContainerRefEntryType.__new__(ArgumentContainerRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentContainerSegmentRefEntryType = frozenset(
    {"shortDescription", "containerRef", "order", "sizeInBits"})


def _r_ArgumentContainerSegmentRefEntryType(
        n: X.Node) -> ArgumentContainerSegmentRefEntryType:
    X.check_attrs(n, _A_ArgumentContainerSegmentRefEntryType)
    v = ArgumentContainerSegmentRefEntryType.__new__(
        ArgumentContainerSegmentRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentDimensionListType = frozenset({})


def _r_ArgumentDimensionListType(n: X.Node) -> ArgumentDimensionListType:
    X.check_attrs(n, _A_ArgumentDimensionListType)
    v = ArgumentDimensionListType.__new__(ArgumentDimensionListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Dimension':
        items.append(_r_ArgumentDimensionType(c.take()))
    if len(items) < 1:
        raise c.too_few('Dimension', 1, len(items))
    v.dimension = items
    c.end()
    return v


_A_ArgumentDimensionType = frozenset({})


def _r_ArgumentDimensionType(n: X.Node) -> ArgumentDimensionType:
    X.check_attrs(n, _A_ArgumentDimensionType)
    v = ArgumentDimensionType.__new__(ArgumentDimensionType)
    c = X.Cursor(n)
    if c.peek() != 'StartingIndex':
        raise c.expected('StartingIndex')
    v.starting_index = _r_ArgumentIntegerValueType(c.take())
    if c.peek() != 'EndingIndex':
        raise c.expected('EndingIndex')
    v.ending_index = _r_ArgumentIntegerValueType(c.take())
    c.end()
    return v


_A_ArgumentDiscreteLookupListType = frozenset({'defaultValue'})


def _r_ArgumentDiscreteLookupListType(
        n: X.Node) -> ArgumentDiscreteLookupListType:
    X.check_attrs(n, _A_ArgumentDiscreteLookupListType)
    v = ArgumentDiscreteLookupListType.__new__(ArgumentDiscreteLookupListType)
    v.default_value = X.req_attr(n, 'defaultValue', X.p_i64)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'DiscreteLookup':
        items.append(_r_ArgumentDiscreteLookupType(c.take()))
    if len(items) < 1:
        raise c.too_few('DiscreteLookup', 1, len(items))
    v.discrete_lookup = items
    c.end()
    return v


_A_ArgumentDiscreteLookupType = frozenset({'value'})


def _r_ArgumentDiscreteLookupType(n: X.Node) -> ArgumentDiscreteLookupType:
    X.check_attrs(n, _A_ArgumentDiscreteLookupType)
    v = ArgumentDiscreteLookupType.__new__(ArgumentDiscreteLookupType)
    v.value = X.req_attr(n, 'value', X.p_i64)
    c = X.Cursor(n)
    v.choice = _r_ArgumentDiscreteLookupChoice(c)
    c.end()
    return v


_A_ArgumentDynamicValueType = frozenset({})


def _r_ArgumentDynamicValueType(n: X.Node) -> ArgumentDynamicValueType:
    X.check_attrs(n, _A_ArgumentDynamicValueType)
    v = ArgumentDynamicValueType.__new__(ArgumentDynamicValueType)
    c = X.Cursor(n)
    v.choice = _r_ArgumentDynamicValueChoice(c)
    v.linear_adjustment = (_r_LinearAdjustmentType(c.take())
                           if c.peek() == "LinearAdjustment" else None)
    c.end()
    return v


_A_ArgumentEnumeratedDataType = frozenset({"shortDescription", "name",
                                           "baseType", "initialValue"})


def _r_ArgumentEnumeratedDataType(n: X.Node) -> ArgumentEnumeratedDataType:
    X.check_attrs(n, _A_ArgumentEnumeratedDataType)
    v = ArgumentEnumeratedDataType.__new__(ArgumentEnumeratedDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentEnumeratedDataChoice(c)
                if c.peek() in ArgumentEnumeratedDataChoice.NAMES else None)
    if c.peek() != 'EnumerationList':
        raise c.expected('EnumerationList')
    v.enumeration_list = _r_EnumerationListType(c.take())
    c.end()
    return v


_A_ArgumentFixedValueEntryType = frozenset({"shortDescription", "name",
                                            "binaryValue", "sizeInBits"})


def _r_ArgumentFixedValueEntryType(n: X.Node) -> ArgumentFixedValueEntryType:
    X.check_attrs(n, _A_ArgumentFixedValueEntryType)
    v = ArgumentFixedValueEntryType.__new__(ArgumentFixedValueEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.binary_value = X.req_attr(n, 'binaryValue', X.p_str)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentFloatDataType = frozenset({"shortDescription", "name", "baseType",
                                      "initialValue", "sizeInBits"})


def _r_ArgumentFloatDataType(n: X.Node) -> ArgumentFloatDataType:
    X.check_attrs(n, _A_ArgumentFloatDataType)
    v = ArgumentFloatDataType.__new__(ArgumentFloatDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_double)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', _p_FloatSizeInBitsType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentFloatDataChoice(c)
                if c.peek() in ArgumentFloatDataChoice.NAMES else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    c.end()
    return v


_A_ArgumentIndirectParameterRefEntryType = frozenset({"shortDescription",
                                                      "aliasNameSpace"})


def _r_ArgumentIndirectParameterRefEntryType(
        n: X.Node) -> ArgumentIndirectParameterRefEntryType:
    X.check_attrs(n, _A_ArgumentIndirectParameterRefEntryType)
    v = ArgumentIndirectParameterRefEntryType.__new__(
        ArgumentIndirectParameterRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.alias_name_space = X.opt_attr(n, 'aliasNameSpace', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'ParameterInstance':
        raise c.expected('ParameterInstance')
    v.parameter_instance = _r_ParameterInstanceRefType(c.take())
    c.end()
    return v


_A_ArgumentInputAlgorithmType = frozenset({'shortDescription', 'name'})


def _r_ArgumentInputAlgorithmType(n: X.Node) -> ArgumentInputAlgorithmType:
    X.check_attrs(n, _A_ArgumentInputAlgorithmType)
    v = ArgumentInputAlgorithmType.__new__(ArgumentInputAlgorithmType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.algorithm_text = (_r_AlgorithmTextType(c.take())
                        if c.peek() == "AlgorithmText" else None)
    v.external_algorithm_set = (_r_ExternalAlgorithmSetType(c.take())
                                if c.peek() == "ExternalAlgorithmSet" else None)
    v.input_set = (_r_ArgumentInputSetType(c.take()) if c.peek() == "InputSet"
                   else None)
    c.end()
    return v


_A_ArgumentInputSetType = frozenset({})


def _r_ArgumentInputSetType(n: X.Node) -> ArgumentInputSetType:
    X.check_attrs(n, _A_ArgumentInputSetType)
    v = ArgumentInputSetType.__new__(ArgumentInputSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ArgumentInputSetChoice.NAMES:
        items.append(_r_ArgumentInputSetChoice(c))
    if len(items) < 1:
        raise c.too_few("InputParameterInstanceRef|InputArgumentInstanceRef|"
                        "Constant", 1, len(items))
    v.items = items
    c.end()
    return v


_A_ArgumentInstanceRefType = frozenset({'argumentRef', 'useCalibratedValue'})


def _r_ArgumentInstanceRefType(n: X.Node) -> ArgumentInstanceRefType:
    X.check_attrs(n, _A_ArgumentInstanceRefType)
    v = ArgumentInstanceRefType.__new__(ArgumentInstanceRefType)
    v.argument_ref = X.req_attr(n, 'argumentRef', X.p_str)
    v.use_calibrated_value = X.opt_attr(n, 'useCalibratedValue', X.p_bool)
    c = X.Cursor(n)
    c.end()
    return v


_A_ArgumentIntegerDataType = frozenset({"shortDescription", "name", "baseType",
                                        "initialValue", "sizeInBits", "signed"})


def _r_ArgumentIntegerDataType(n: X.Node) -> ArgumentIntegerDataType:
    X.check_attrs(n, _A_ArgumentIntegerDataType)
    v = ArgumentIntegerDataType.__new__(ArgumentIntegerDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_i64)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', X.p_i64)
    v.signed = X.opt_attr(n, 'signed', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentIntegerDataChoice(c)
                if c.peek() in ArgumentIntegerDataChoice.NAMES else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    c.end()
    return v


_A_ArgumentIntegerValueType = frozenset({})


def _r_ArgumentIntegerValueType(n: X.Node) -> ArgumentIntegerValueType:
    X.check_attrs(n, _A_ArgumentIntegerValueType)
    v = ArgumentIntegerValueType.__new__(ArgumentIntegerValueType)
    c = X.Cursor(n)
    v.choice = _r_ArgumentIntegerValueChoice(c)
    c.end()
    return v


_A_ArgumentListType = frozenset({})


def _r_ArgumentListType(n: X.Node) -> ArgumentListType:
    X.check_attrs(n, _A_ArgumentListType)
    v = ArgumentListType.__new__(ArgumentListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Argument':
        items.append(_r_ArgumentType(c.take()))
    if len(items) < 1:
        raise c.too_few('Argument', 1, len(items))
    v.argument = items
    c.end()
    return v


_A_ArgumentLocationInContainerInBitsType = frozenset({'referenceLocation'})


def _r_ArgumentLocationInContainerInBitsType(
        n: X.Node) -> ArgumentLocationInContainerInBitsType:
    X.check_attrs(n, _A_ArgumentLocationInContainerInBitsType)
    v = ArgumentLocationInContainerInBitsType.__new__(
        ArgumentLocationInContainerInBitsType)
    v.reference_location = X.opt_attr(n, "referenceLocation",
                                      _p_ReferenceLocationType)
    c = X.Cursor(n)
    v.choice = _r_ArgumentLocationInContainerInBitsChoice(c)
    c.end()
    return v


_A_ArgumentMatchCriteriaType = frozenset({})


def _r_ArgumentMatchCriteriaType(n: X.Node) -> ArgumentMatchCriteriaType:
    X.check_attrs(n, _A_ArgumentMatchCriteriaType)
    v = ArgumentMatchCriteriaType.__new__(ArgumentMatchCriteriaType)
    c = X.Cursor(n)
    v.choice = _r_ArgumentMatchCriteriaChoice(c)
    c.end()
    return v


_A_ArgumentMathOperationType = frozenset({})


def _r_ArgumentMathOperationType(n: X.Node) -> ArgumentMathOperationType:
    X.check_attrs(n, _A_ArgumentMathOperationType)
    v = ArgumentMathOperationType.__new__(ArgumentMathOperationType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ArgumentMathOperationChoice.NAMES:
        items.append(_r_ArgumentMathOperationChoice(c))
    if len(items) < 1:
        raise c.too_few("ValueOperand|ThisParameterOperand|Operator|"
                        "ParameterInstanceRefOperand|"
                        "ArgumentInstanceRefOperand", 1, len(items))
    v.items = items
    c.end()
    return v


_A_ArgumentORedConditionsType = frozenset({})


def _r_ArgumentORedConditionsType(n: X.Node) -> ArgumentORedConditionsType:
    X.check_attrs(n, _A_ArgumentORedConditionsType)
    v = ArgumentORedConditionsType.__new__(ArgumentORedConditionsType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ArgumentORedConditionsChoice.NAMES:
        items.append(_r_ArgumentORedConditionsChoice(c))
    if len(items) < 2:
        raise c.too_few('Condition|ANDedConditions', 2, len(items))
    v.items = items
    c.end()
    return v


_A_ArgumentParameterRefEntryType = frozenset({"shortDescription",
                                              "parameterRef"})


def _r_ArgumentParameterRefEntryType(
        n: X.Node) -> ArgumentParameterRefEntryType:
    X.check_attrs(n, _A_ArgumentParameterRefEntryType)
    v = ArgumentParameterRefEntryType.__new__(ArgumentParameterRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentParameterSegmentRefEntryType = frozenset(
    {"shortDescription", "parameterRef", "order", "sizeInBits"})


def _r_ArgumentParameterSegmentRefEntryType(
        n: X.Node) -> ArgumentParameterSegmentRefEntryType:
    X.check_attrs(n, _A_ArgumentParameterSegmentRefEntryType)
    v = ArgumentParameterSegmentRefEntryType.__new__(
        ArgumentParameterSegmentRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentRelativeTimeDataType = frozenset({"shortDescription", "name",
                                             "baseType", "initialValue"})


def _r_ArgumentRelativeTimeDataType(n: X.Node) -> ArgumentRelativeTimeDataType:
    X.check_attrs(n, _A_ArgumentRelativeTimeDataType)
    v = ArgumentRelativeTimeDataType.__new__(ArgumentRelativeTimeDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_ArgumentRepeatType = frozenset({})


def _r_ArgumentRepeatType(n: X.Node) -> ArgumentRepeatType:
    X.check_attrs(n, _A_ArgumentRepeatType)
    v = ArgumentRepeatType.__new__(ArgumentRepeatType)
    c = X.Cursor(n)
    if c.peek() != 'Count':
        raise c.expected('Count')
    v.count = _r_ArgumentIntegerValueType(c.take())
    v.offset = (_r_ArgumentIntegerValueType(c.take()) if c.peek() == "Offset"
                else None)
    c.end()
    return v


_A_ArgumentRestrictionListType = frozenset({})


def _r_ArgumentRestrictionListType(n: X.Node) -> ArgumentRestrictionListType:
    X.check_attrs(n, _A_ArgumentRestrictionListType)
    v = ArgumentRestrictionListType.__new__(ArgumentRestrictionListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ArgumentRestriction':
        items.append(_r_ArgumentAssignmentType(c.take()))
    if len(items) < 1:
        raise c.too_few('ArgumentRestriction', 1, len(items))
    v.argument_restriction = items
    c.end()
    return v


_A_ArgumentSequenceEntryType = frozenset({'shortDescription'})


def _r_ArgumentSequenceEntryType(n: X.Node) -> ArgumentSequenceEntryType:
    X.check_attrs(n, _A_ArgumentSequenceEntryType)
    v = ArgumentSequenceEntryType.__new__(ArgumentSequenceEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentStreamSegmentEntryType = frozenset({"shortDescription", "streamRef",
                                               "order", "sizeInBits"})


def _r_ArgumentStreamSegmentEntryType(
        n: X.Node) -> ArgumentStreamSegmentEntryType:
    X.check_attrs(n, _A_ArgumentStreamSegmentEntryType)
    v = ArgumentStreamSegmentEntryType.__new__(ArgumentStreamSegmentEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.stream_ref = X.req_attr(n, 'streamRef', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_ArgumentLocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_ArgumentRepeatType(c.take())
                      if c.peek() == "RepeatEntry" else None)
    v.include_condition = (_r_ArgumentMatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentStringDataEncodingType = frozenset({"bitOrder", "byteOrder",
                                               "encoding"})


def _r_ArgumentStringDataEncodingType(
        n: X.Node) -> ArgumentStringDataEncodingType:
    X.check_attrs(n, _A_ArgumentStringDataEncodingType)
    v = ArgumentStringDataEncodingType.__new__(ArgumentStringDataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    v.encoding = X.opt_attr(n, 'encoding', _p_StringEncodingType)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    v.choice = _r_ArgumentStringDataEncodingChoice(c)
    c.end()
    return v


_A_ArgumentStringDataType = frozenset({"shortDescription", "name", "baseType",
                                       "initialValue", "restrictionPattern",
                                       "characterWidth"})


def _r_ArgumentStringDataType(n: X.Node) -> ArgumentStringDataType:
    X.check_attrs(n, _A_ArgumentStringDataType)
    v = ArgumentStringDataType.__new__(ArgumentStringDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.restriction_pattern = X.opt_attr(n, 'restrictionPattern', X.p_str)
    v.character_width = X.opt_attr(n, 'characterWidth', _p_CharacterWidthType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_ArgumentStringDataChoice(c)
                if c.peek() in ArgumentStringDataChoice.NAMES else None)
    v.size_range_in_characters = (_r_IntegerRangeType(c.take())
                                  if c.peek() == "SizeRangeInCharacters"
                                  else None)
    c.end()
    return v


_A_ArgumentType = frozenset({"shortDescription", "name", "argumentTypeRef",
                             "initialValue"})


def _r_ArgumentType(n: X.Node) -> ArgumentType:
    X.check_attrs(n, _A_ArgumentType)
    v = ArgumentType.__new__(ArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.argument_type_ref = X.req_attr(n, 'argumentTypeRef', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArgumentTypeSetType = frozenset({})


def _r_ArgumentTypeSetType(n: X.Node) -> ArgumentTypeSetType:
    X.check_attrs(n, _A_ArgumentTypeSetType)
    v = ArgumentTypeSetType.__new__(ArgumentTypeSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ArgumentTypeSetChoice.NAMES:
        items.append(_r_ArgumentTypeSetChoice(c))
    if len(items) < 1:
        raise c.too_few(
            "StringArgumentType|EnumeratedArgumentType|IntegerArgumentType|"
            "BinaryArgumentType|FloatArgumentType|BooleanArgumentType|"
            "RelativeTimeArgumentType|AbsoluteTimeArgumentType|"
            "ArrayArgumentType|AggregateArgumentType", 1, len(items))
    v.items = items
    c.end()
    return v


_A_ArgumentVariableStringType = frozenset({'maxSizeInBits'})


def _r_ArgumentVariableStringType(n: X.Node) -> ArgumentVariableStringType:
    X.check_attrs(n, _A_ArgumentVariableStringType)
    v = ArgumentVariableStringType.__new__(ArgumentVariableStringType)
    v.max_size_in_bits = X.req_attr(n, 'maxSizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.choice1 = (_r_ArgumentVariableStringChoice1(c)
                 if c.peek() in ArgumentVariableStringChoice1.NAMES else None)
    v.choice2 = _r_ArgumentVariableStringChoice2(c)
    c.end()
    return v


_A_ArrayArgumentType = frozenset({"shortDescription", "name", "arrayTypeRef",
                                  "initialValue"})


def _r_ArrayArgumentType(n: X.Node) -> ArrayArgumentType:
    X.check_attrs(n, _A_ArrayArgumentType)
    v = ArrayArgumentType.__new__(ArrayArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.array_type_ref = X.req_attr(n, 'arrayTypeRef', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'DimensionList':
        raise c.expected('DimensionList')
    v.dimension_list = _r_ArgumentDimensionListType(c.take())
    c.end()
    return v


_A_ArrayDataTypeType = frozenset({"shortDescription", "name", "arrayTypeRef",
                                  "initialValue"})


def _r_ArrayDataTypeType(n: X.Node) -> ArrayDataTypeType:
    X.check_attrs(n, _A_ArrayDataTypeType)
    v = ArrayDataTypeType.__new__(ArrayDataTypeType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.array_type_ref = X.req_attr(n, 'arrayTypeRef', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ArrayParameterRefEntryType = frozenset({'shortDescription', 'parameterRef'})


def _r_ArrayParameterRefEntryType(n: X.Node) -> ArrayParameterRefEntryType:
    X.check_attrs(n, _A_ArrayParameterRefEntryType)
    v = ArrayParameterRefEntryType.__new__(ArrayParameterRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.dimension_list = (_r_DimensionListType(c.take())
                        if c.peek() == "DimensionList" else None)
    c.end()
    return v


_A_ArrayParameterType = frozenset({"shortDescription", "name", "arrayTypeRef",
                                   "initialValue"})


def _r_ArrayParameterType(n: X.Node) -> ArrayParameterType:
    X.check_attrs(n, _A_ArrayParameterType)
    v = ArrayParameterType.__new__(ArrayParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.array_type_ref = X.req_attr(n, 'arrayTypeRef', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'DimensionList':
        raise c.expected('DimensionList')
    v.dimension_list = _r_DimensionListType(c.take())
    c.end()
    return v


_A_AuthorSetType = frozenset({})


def _r_AuthorSetType(n: X.Node) -> AuthorSetType:
    X.check_attrs(n, _A_AuthorSetType)
    v = AuthorSetType.__new__(AuthorSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Author':
        items.append(X.leaf(c.take(), X.p_str))
    v.author = items
    c.end()
    return v


_A_AutoInvertType = frozenset({'badFramesToAutoInvert'})


def _r_AutoInvertType(n: X.Node) -> AutoInvertType:
    X.check_attrs(n, _A_AutoInvertType)
    v = AutoInvertType.__new__(AutoInvertType)
    v.bad_frames_to_auto_invert = X.opt_attr(n, "badFramesToAutoInvert",
                                             X.p_i64)
    c = X.Cursor(n)
    v.invert_algorithm = (_r_InputAlgorithmType(c.take())
                          if c.peek() == "InvertAlgorithm" else None)
    c.end()
    return v


_A_BaseAlarmType = frozenset({'name', 'shortDescription'})


def _r_BaseAlarmType(n: X.Node) -> BaseAlarmType:
    X.check_attrs(n, _A_BaseAlarmType)
    v = BaseAlarmType.__new__(BaseAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_BaseCalibratorType = frozenset({'name', 'shortDescription'})


def _r_BaseCalibratorType(n: X.Node) -> BaseCalibratorType:
    X.check_attrs(n, _A_BaseCalibratorType)
    v = BaseCalibratorType.__new__(BaseCalibratorType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_BaseComparisonType = frozenset({})


def _r_BaseComparisonType(n: X.Node) -> BaseComparisonType:
    X.check_attrs(n, _A_BaseComparisonType)
    v = BaseComparisonType.__new__(BaseComparisonType)
    c = X.Cursor(n)
    c.end()
    return v


_A_BaseConditionsType = frozenset({})


def _r_BaseConditionsType(n: X.Node) -> BaseConditionsType:
    X.check_attrs(n, _A_BaseConditionsType)
    v = BaseConditionsType.__new__(BaseConditionsType)
    c = X.Cursor(n)
    c.end()
    return v


_A_BaseContainerType = frozenset({'containerRef'})


def _r_BaseContainerType(n: X.Node) -> BaseContainerType:
    X.check_attrs(n, _A_BaseContainerType)
    v = BaseContainerType.__new__(BaseContainerType)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    c = X.Cursor(n)
    v.restriction_criteria = (_r_RestrictionCriteriaType(c.take())
                              if c.peek() == "RestrictionCriteria" else None)
    c.end()
    return v


_A_BaseDataType = frozenset({'shortDescription', 'name', 'baseType'})


def _r_BaseDataType(n: X.Node) -> BaseDataType:
    X.check_attrs(n, _A_BaseDataType)
    v = BaseDataType.__new__(BaseDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BaseDataChoice(c) if c.peek() in BaseDataChoice.NAMES
                else None)
    c.end()
    return v


_A_BaseMetaCommandType = frozenset({'metaCommandRef'})


def _r_BaseMetaCommandType(n: X.Node) -> BaseMetaCommandType:
    X.check_attrs(n, _A_BaseMetaCommandType)
    v = BaseMetaCommandType.__new__(BaseMetaCommandType)
    v.meta_command_ref = X.req_attr(n, 'metaCommandRef', X.p_str)
    c = X.Cursor(n)
    v.argument_assignment_list = (_r_ArgumentAssignmentListType(c.take())
                                  if c.peek() == "ArgumentAssignmentList"
                                  else None)
    c.end()
    return v


_A_BaseTimeDataType = frozenset({'shortDescription', 'name', 'baseType'})


def _r_BaseTimeDataType(n: X.Node) -> BaseTimeDataType:
    X.check_attrs(n, _A_BaseTimeDataType)
    v = BaseTimeDataType.__new__(BaseTimeDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_BaseTriggerType = frozenset({})


def _r_BaseTriggerType(n: X.Node) -> BaseTriggerType:
    X.check_attrs(n, _A_BaseTriggerType)
    v = BaseTriggerType.__new__(BaseTriggerType)
    c = X.Cursor(n)
    c.end()
    return v


_A_BinaryAlarmType = frozenset({"name", "shortDescription", "minViolations",
                                "minConformance", "disabled"})


def _r_BinaryAlarmType(n: X.Node) -> BinaryAlarmType:
    X.check_attrs(n, _A_BinaryAlarmType)
    v = BinaryAlarmType.__new__(BinaryAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_BinaryAlarmChoice(c) if c.peek() in BinaryAlarmChoice.NAMES
                else None)
    c.end()
    return v


_A_BinaryArgumentType = frozenset({"shortDescription", "name", "baseType",
                                   "initialValue"})


def _r_BinaryArgumentType(n: X.Node) -> BinaryArgumentType:
    X.check_attrs(n, _A_BinaryArgumentType)
    v = BinaryArgumentType.__new__(BinaryArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BinaryArgumentChoice(c)
                if c.peek() in BinaryArgumentChoice.NAMES else None)
    c.end()
    return v


_A_BinaryContextAlarmListType = frozenset({})


def _r_BinaryContextAlarmListType(n: X.Node) -> BinaryContextAlarmListType:
    X.check_attrs(n, _A_BinaryContextAlarmListType)
    v = BinaryContextAlarmListType.__new__(BinaryContextAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextAlarm':
        items.append(_r_BinaryContextAlarmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextAlarm', 1, len(items))
    v.context_alarm = items
    c.end()
    return v


_A_BinaryContextAlarmType = frozenset(
    {"name", "shortDescription", "minViolations", "minConformance", "disabled"})


def _r_BinaryContextAlarmType(n: X.Node) -> BinaryContextAlarmType:
    X.check_attrs(n, _A_BinaryContextAlarmType)
    v = BinaryContextAlarmType.__new__(BinaryContextAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_BinaryContextAlarmChoice(c)
                if c.peek() in BinaryContextAlarmChoice.NAMES else None)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    c.end()
    return v


_A_BinaryDataEncodingType = frozenset({'bitOrder', 'byteOrder'})


def _r_BinaryDataEncodingType(n: X.Node) -> BinaryDataEncodingType:
    X.check_attrs(n, _A_BinaryDataEncodingType)
    v = BinaryDataEncodingType.__new__(BinaryDataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    if c.peek() != 'SizeInBits':
        raise c.expected('SizeInBits')
    v.size_in_bits = _r_IntegerValueType(c.take())
    v.from_binary_transform_algorithm = _r_InputAlgorithmType(
        c.take()) if c.peek() == 'FromBinaryTransformAlgorithm' else None
    v.to_binary_transform_algorithm = _r_InputAlgorithmType(
        c.take()) if c.peek() == 'ToBinaryTransformAlgorithm' else None
    c.end()
    return v


_A_BinaryDataType = frozenset({"shortDescription", "name", "baseType",
                               "initialValue"})


def _r_BinaryDataType(n: X.Node) -> BinaryDataType:
    X.check_attrs(n, _A_BinaryDataType)
    v = BinaryDataType.__new__(BinaryDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BinaryDataChoice(c) if c.peek() in BinaryDataChoice.NAMES
                else None)
    c.end()
    return v


_A_BinaryParameterType = frozenset({"shortDescription", "name", "baseType",
                                    "initialValue"})


def _r_BinaryParameterType(n: X.Node) -> BinaryParameterType:
    X.check_attrs(n, _A_BinaryParameterType)
    v = BinaryParameterType.__new__(BinaryParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BinaryParameterChoice(c)
                if c.peek() in BinaryParameterChoice.NAMES else None)
    v.default_alarm = (_r_BinaryAlarmType(c.take())
                       if c.peek() == "DefaultAlarm" else None)
    v.context_alarm_list = (_r_BinaryContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_BlockMetaCommandType = frozenset({'shortDescription', 'name'})


def _r_BlockMetaCommandType(n: X.Node) -> BlockMetaCommandType:
    X.check_attrs(n, _A_BlockMetaCommandType)
    v = BlockMetaCommandType.__new__(BlockMetaCommandType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'MetaCommandStepList':
        raise c.expected('MetaCommandStepList')
    v.meta_command_step_list = _r_MetaCommandStepListType(c.take())
    c.end()
    return v


_A_BooleanAlarmType = frozenset({"name", "shortDescription", "minViolations",
                                 "minConformance", "disabled"})


def _r_BooleanAlarmType(n: X.Node) -> BooleanAlarmType:
    X.check_attrs(n, _A_BooleanAlarmType)
    v = BooleanAlarmType.__new__(BooleanAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_BooleanAlarmChoice(c) if c.peek() in BooleanAlarmChoice.NAMES
                else None)
    c.end()
    return v


_A_BooleanArgumentType = frozenset({"shortDescription", "name", "baseType",
                                    "initialValue", "oneStringValue",
                                    "zeroStringValue"})


def _r_BooleanArgumentType(n: X.Node) -> BooleanArgumentType:
    X.check_attrs(n, _A_BooleanArgumentType)
    v = BooleanArgumentType.__new__(BooleanArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.one_string_value = X.opt_attr(n, 'oneStringValue', X.p_str)
    v.zero_string_value = X.opt_attr(n, 'zeroStringValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BooleanArgumentChoice(c)
                if c.peek() in BooleanArgumentChoice.NAMES else None)
    c.end()
    return v


_A_BooleanContextAlarmListType = frozenset({})


def _r_BooleanContextAlarmListType(n: X.Node) -> BooleanContextAlarmListType:
    X.check_attrs(n, _A_BooleanContextAlarmListType)
    v = BooleanContextAlarmListType.__new__(BooleanContextAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextAlarm':
        items.append(_r_BooleanContextAlarmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextAlarm', 1, len(items))
    v.context_alarm = items
    c.end()
    return v


_A_BooleanContextAlarmType = frozenset(
    {"name", "shortDescription", "minViolations", "minConformance", "disabled"})


def _r_BooleanContextAlarmType(n: X.Node) -> BooleanContextAlarmType:
    X.check_attrs(n, _A_BooleanContextAlarmType)
    v = BooleanContextAlarmType.__new__(BooleanContextAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_BooleanContextAlarmChoice(c)
                if c.peek() in BooleanContextAlarmChoice.NAMES else None)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    c.end()
    return v


_A_BooleanDataType = frozenset({"shortDescription", "name", "baseType",
                                "initialValue", "oneStringValue",
                                "zeroStringValue"})


def _r_BooleanDataType(n: X.Node) -> BooleanDataType:
    X.check_attrs(n, _A_BooleanDataType)
    v = BooleanDataType.__new__(BooleanDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.one_string_value = X.opt_attr(n, 'oneStringValue', X.p_str)
    v.zero_string_value = X.opt_attr(n, 'zeroStringValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BooleanDataChoice(c) if c.peek() in BooleanDataChoice.NAMES
                else None)
    c.end()
    return v


_A_BooleanExpressionType = frozenset({})


def _r_BooleanExpressionType(n: X.Node) -> BooleanExpressionType:
    X.check_attrs(n, _A_BooleanExpressionType)
    v = BooleanExpressionType.__new__(BooleanExpressionType)
    c = X.Cursor(n)
    v.choice = _r_BooleanExpressionChoice(c)
    c.end()
    return v


_A_BooleanParameterType = frozenset({"shortDescription", "name", "baseType",
                                     "initialValue", "oneStringValue",
                                     "zeroStringValue"})


def _r_BooleanParameterType(n: X.Node) -> BooleanParameterType:
    X.check_attrs(n, _A_BooleanParameterType)
    v = BooleanParameterType.__new__(BooleanParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.one_string_value = X.opt_attr(n, 'oneStringValue', X.p_str)
    v.zero_string_value = X.opt_attr(n, 'zeroStringValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_BooleanParameterChoice(c)
                if c.peek() in BooleanParameterChoice.NAMES else None)
    v.default_alarm = (_r_BooleanAlarmType(c.take())
                       if c.peek() == "DefaultAlarm" else None)
    v.context_alarm_list = (_r_BooleanContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_ByteType = frozenset({'byteSignificance'})


def _r_ByteType(n: X.Node) -> ByteType:
    X.check_attrs(n, _A_ByteType)
    v = ByteType.__new__(ByteType)
    v.byte_significance = X.req_attr(n, 'byteSignificance', X.p_i64)
    c = X.Cursor(n)
    c.end()
    return v


_A_CRCType = frozenset({"width", "reflectData", "reflectRemainder", "direction",
                        "bitsFromReference", "reference", "parameterRef"})


def _r_CRCType(n: X.Node) -> CRCType:
    X.check_attrs(n, _A_CRCType)
    v = CRCType.__new__(CRCType)
    v.width = X.req_attr(n, 'width', X.p_i64)
    v.reflect_data = X.opt_attr(n, 'reflectData', X.p_bool)
    v.reflect_remainder = X.opt_attr(n, 'reflectRemainder', X.p_bool)
    v.direction = X.opt_attr(n, 'direction', _p_BitOrderType)
    v.bits_from_reference = X.opt_attr(n, 'bitsFromReference', X.p_i64)
    v.reference = X.opt_attr(n, 'reference', _p_ReferencePointType)
    v.parameter_ref = X.opt_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    if c.peek() != 'Polynomial':
        raise c.expected('Polynomial')
    v.polynomial = X.leaf(c.take(), X.p_str)
    v.init_remainder = (X.leaf(c.take(), X.p_str) if c.peek() == "InitRemainder"
                        else None)
    v.final_xor = X.leaf(c.take(), X.p_str) if c.peek() == 'FinalXOR' else None
    c.end()
    return v


_A_CalibratorType = frozenset({'name', 'shortDescription'})


def _r_CalibratorType(n: X.Node) -> CalibratorType:
    X.check_attrs(n, _A_CalibratorType)
    v = CalibratorType.__new__(CalibratorType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = _r_CalibratorChoice(c)
    c.end()
    return v


_A_ChangeAlarmRangesType = frozenset(
    {"name", "shortDescription", "rangeForm", "changeType", "changeBasis",
     "spanOfInterestInSamples", "spanOfInterestInSeconds"})


def _r_ChangeAlarmRangesType(n: X.Node) -> ChangeAlarmRangesType:
    X.check_attrs(n, _A_ChangeAlarmRangesType)
    v = ChangeAlarmRangesType.__new__(ChangeAlarmRangesType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.range_form = X.opt_attr(n, 'rangeForm', _p_RangeFormType)
    v.change_type = X.opt_attr(n, 'changeType', _p_ChangeSpanType)
    v.change_basis = X.opt_attr(n, 'changeBasis', _p_ChangeBasisType)
    v.span_of_interest_in_samples = X.opt_attr(n, "spanOfInterestInSamples",
                                               X.p_i64)
    v.span_of_interest_in_seconds = X.opt_attr(n, "spanOfInterestInSeconds",
                                               X.p_double)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.watch_range = (_r_FloatRangeType(c.take()) if c.peek() == "WatchRange"
                     else None)
    v.warning_range = (_r_FloatRangeType(c.take()) if c.peek() == "WarningRange"
                       else None)
    v.distress_range = (_r_FloatRangeType(c.take())
                        if c.peek() == "DistressRange" else None)
    v.critical_range = (_r_FloatRangeType(c.take())
                        if c.peek() == "CriticalRange" else None)
    v.severe_range = (_r_FloatRangeType(c.take()) if c.peek() == "SevereRange"
                      else None)
    c.end()
    return v


_A_ChangeValueType = frozenset({'value'})


def _r_ChangeValueType(n: X.Node) -> ChangeValueType:
    X.check_attrs(n, _A_ChangeValueType)
    v = ChangeValueType.__new__(ChangeValueType)
    v.value = X.req_attr(n, 'value', X.p_double)
    c = X.Cursor(n)
    c.end()
    return v


_A_CheckWindowAlgorithmsType = frozenset({})


def _r_CheckWindowAlgorithmsType(n: X.Node) -> CheckWindowAlgorithmsType:
    X.check_attrs(n, _A_CheckWindowAlgorithmsType)
    v = CheckWindowAlgorithmsType.__new__(CheckWindowAlgorithmsType)
    c = X.Cursor(n)
    if c.peek() != 'StartCheck':
        raise c.expected('StartCheck')
    v.start_check = _r_InputAlgorithmType(c.take())
    if c.peek() != 'StopTime':
        raise c.expected('StopTime')
    v.stop_time = _r_InputAlgorithmType(c.take())
    c.end()
    return v


_A_CheckWindowType = frozenset({"timeToStartChecking", "timeToStopChecking",
                                "timeWindowIsRelativeTo"})


def _r_CheckWindowType(n: X.Node) -> CheckWindowType:
    X.check_attrs(n, _A_CheckWindowType)
    v = CheckWindowType.__new__(CheckWindowType)
    v.time_to_start_checking = X.opt_attr(n, 'timeToStartChecking', X.p_str)
    v.time_to_stop_checking = X.req_attr(n, 'timeToStopChecking', X.p_str)
    v.time_window_is_relative_to = X.opt_attr(n, "timeWindowIsRelativeTo",
                                              _p_TimeWindowIsRelativeToType)
    c = X.Cursor(n)
    c.end()
    return v


_A_ChecksumType = frozenset({"bitsFromReference", "reference", "name",
                             "hashSizeInBits", "parameterRef"})


def _r_ChecksumType(n: X.Node) -> ChecksumType:
    X.check_attrs(n, _A_ChecksumType)
    v = ChecksumType.__new__(ChecksumType)
    v.bits_from_reference = X.opt_attr(n, 'bitsFromReference', X.p_i64)
    v.reference = X.opt_attr(n, 'reference', _p_ReferencePointType)
    v.name = X.req_attr(n, 'name', _p_ChecksumnameType)
    v.hash_size_in_bits = X.opt_attr(n, 'hashSizeInBits', X.p_i64)
    v.parameter_ref = X.opt_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    v.input_algorithm = (_r_InputAlgorithmType(c.take())
                         if c.peek() == "InputAlgorithm" else None)
    c.end()
    return v


_A_CommandContainerEntryListType = frozenset({})


def _r_CommandContainerEntryListType(
        n: X.Node) -> CommandContainerEntryListType:
    X.check_attrs(n, _A_CommandContainerEntryListType)
    v = CommandContainerEntryListType.__new__(CommandContainerEntryListType)
    c = X.Cursor(n)
    items = []
    while c.peek() in CommandContainerEntryListChoice.NAMES:
        items.append(_r_CommandContainerEntryListChoice(c))
    v.items = items
    c.end()
    return v


_A_CommandContainerSetType = frozenset({})


def _r_CommandContainerSetType(n: X.Node) -> CommandContainerSetType:
    X.check_attrs(n, _A_CommandContainerSetType)
    v = CommandContainerSetType.__new__(CommandContainerSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'CommandContainer':
        items.append(_r_SequenceContainerType(c.take()))
    if len(items) < 1:
        raise c.too_few('CommandContainer', 1, len(items))
    v.command_container = items
    c.end()
    return v


_A_CommandContainerType = frozenset({'shortDescription', 'name'})


def _r_CommandContainerType(n: X.Node) -> CommandContainerType:
    X.check_attrs(n, _A_CommandContainerType)
    v = CommandContainerType.__new__(CommandContainerType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.default_rate_in_stream = (_r_RateInStreamType(c.take())
                                if c.peek() == "DefaultRateInStream" else None)
    v.rate_in_stream_set = (_r_RateInStreamSetType(c.take())
                            if c.peek() == "RateInStreamSet" else None)
    v.binary_encoding = (_r_ContainerBinaryDataEncodingType(c.take())
                         if c.peek() == "BinaryEncoding" else None)
    if c.peek() != 'EntryList':
        raise c.expected('EntryList')
    v.entry_list = _r_CommandContainerEntryListType(c.take())
    v.base_container = (_r_BaseContainerType(c.take())
                        if c.peek() == "BaseContainer" else None)
    c.end()
    return v


_A_CommandMetaDataType = frozenset({})


def _r_CommandMetaDataType(n: X.Node) -> CommandMetaDataType:
    X.check_attrs(n, _A_CommandMetaDataType)
    v = CommandMetaDataType.__new__(CommandMetaDataType)
    c = X.Cursor(n)
    v.parameter_type_set = (_r_ParameterTypeSetType(c.take())
                            if c.peek() == "ParameterTypeSet" else None)
    v.parameter_set = (_r_ParameterSetType(c.take())
                       if c.peek() == "ParameterSet" else None)
    v.argument_type_set = (_r_ArgumentTypeSetType(c.take())
                           if c.peek() == "ArgumentTypeSet" else None)
    v.meta_command_set = (_r_MetaCommandSetType(c.take())
                          if c.peek() == "MetaCommandSet" else None)
    v.command_container_set = (_r_CommandContainerSetType(c.take())
                               if c.peek() == "CommandContainerSet" else None)
    v.stream_set = (_r_StreamSetType(c.take()) if c.peek() == "StreamSet"
                    else None)
    v.algorithm_set = (_r_AlgorithmSetType(c.take())
                       if c.peek() == "AlgorithmSet" else None)
    c.end()
    return v


_A_CommandVerifierType = frozenset({'shortDescription', 'name'})


def _r_CommandVerifierType(n: X.Node) -> CommandVerifierType:
    X.check_attrs(n, _A_CommandVerifierType)
    v = CommandVerifierType.__new__(CommandVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_CommandVerifierChoice1(c)
    v.choice2 = _r_CommandVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_ComparisonCheckType = frozenset({})


def _r_ComparisonCheckType(n: X.Node) -> ComparisonCheckType:
    X.check_attrs(n, _A_ComparisonCheckType)
    v = ComparisonCheckType.__new__(ComparisonCheckType)
    c = X.Cursor(n)
    if c.peek() != 'ParameterInstanceRef':
        raise c.expected('ParameterInstanceRef')
    v.parameter_instance_ref = _r_ParameterInstanceRefType(c.take())
    if c.peek() != 'ComparisonOperator':
        raise c.expected('ComparisonOperator')
    v.comparison_operator = X.leaf(c.take(), _p_ComparisonOperatorsType)
    v.choice = _r_ComparisonCheckChoice(c)
    c.end()
    return v


_A_ComparisonListType = frozenset({})


def _r_ComparisonListType(n: X.Node) -> ComparisonListType:
    X.check_attrs(n, _A_ComparisonListType)
    v = ComparisonListType.__new__(ComparisonListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Comparison':
        items.append(_r_ComparisonType(c.take()))
    if len(items) < 1:
        raise c.too_few('Comparison', 1, len(items))
    v.comparison = items
    c.end()
    return v


_A_ComparisonType = frozenset({"parameterRef", "instance", "useCalibratedValue",
                               "comparisonOperator", "value"})


def _r_ComparisonType(n: X.Node) -> ComparisonType:
    X.check_attrs(n, _A_ComparisonType)
    v = ComparisonType.__new__(ComparisonType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.instance = X.opt_attr(n, 'instance', X.p_i64)
    v.use_calibrated_value = X.opt_attr(n, 'useCalibratedValue', X.p_bool)
    v.comparison_operator = X.opt_attr(n, "comparisonOperator",
                                       _p_ComparisonOperatorsType)
    v.value = X.req_attr(n, 'value', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_CompleteVerifierType = frozenset({'shortDescription', 'name'})


def _r_CompleteVerifierType(n: X.Node) -> CompleteVerifierType:
    X.check_attrs(n, _A_CompleteVerifierType)
    v = CompleteVerifierType.__new__(CompleteVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_CompleteVerifierChoice1(c)
    v.choice2 = _r_CompleteVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    v.return_parm_ref = (_r_ParameterRefType(c.take())
                         if c.peek() == "ReturnParmRef" else None)
    c.end()
    return v


_A_ConstantType = frozenset({'constantName', 'value'})


def _r_ConstantType(n: X.Node) -> ConstantType:
    X.check_attrs(n, _A_ConstantType)
    v = ConstantType.__new__(ConstantType)
    v.constant_name = X.req_attr(n, 'constantName', X.p_str)
    v.value = X.req_attr(n, 'value', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_ContainerBinaryDataEncodingType = frozenset({})


def _r_ContainerBinaryDataEncodingType(
        n: X.Node) -> ContainerBinaryDataEncodingType:
    X.check_attrs(n, _A_ContainerBinaryDataEncodingType)
    v = ContainerBinaryDataEncodingType.__new__(ContainerBinaryDataEncodingType)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    v.size_in_bits = (_r_IntegerValueType(c.take()) if c.peek() == "SizeInBits"
                      else None)
    v.from_binary_transform_algorithm = _r_InputAlgorithmType(
        c.take()) if c.peek() == 'FromBinaryTransformAlgorithm' else None
    v.to_binary_transform_algorithm = _r_InputAlgorithmType(
        c.take()) if c.peek() == 'ToBinaryTransformAlgorithm' else None
    c.end()
    return v


_A_ContainerRefEntryType = frozenset({'shortDescription', 'containerRef'})


def _r_ContainerRefEntryType(n: X.Node) -> ContainerRefEntryType:
    X.check_attrs(n, _A_ContainerRefEntryType)
    v = ContainerRefEntryType.__new__(ContainerRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ContainerRefSetType = frozenset({})


def _r_ContainerRefSetType(n: X.Node) -> ContainerRefSetType:
    X.check_attrs(n, _A_ContainerRefSetType)
    v = ContainerRefSetType.__new__(ContainerRefSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContainerRef':
        items.append(_r_ContainerRefType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContainerRef', 1, len(items))
    v.container_ref = items
    c.end()
    return v


_A_ContainerRefType = frozenset({'containerRef'})


def _r_ContainerRefType(n: X.Node) -> ContainerRefType:
    X.check_attrs(n, _A_ContainerRefType)
    v = ContainerRefType.__new__(ContainerRefType)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_ContainerSegmentRefEntryType = frozenset({"shortDescription", "containerRef",
                                             "order", "sizeInBits"})


def _r_ContainerSegmentRefEntryType(n: X.Node) -> ContainerSegmentRefEntryType:
    X.check_attrs(n, _A_ContainerSegmentRefEntryType)
    v = ContainerSegmentRefEntryType.__new__(ContainerSegmentRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ContainerSetType = frozenset({})


def _r_ContainerSetType(n: X.Node) -> ContainerSetType:
    X.check_attrs(n, _A_ContainerSetType)
    v = ContainerSetType.__new__(ContainerSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'SequenceContainer':
        items.append(_r_SequenceContainerType(c.take()))
    if len(items) < 1:
        raise c.too_few('SequenceContainer', 1, len(items))
    v.sequence_container = items
    c.end()
    return v


_A_ContainerType = frozenset({'shortDescription', 'name'})


def _r_ContainerType(n: X.Node) -> ContainerType:
    X.check_attrs(n, _A_ContainerType)
    v = ContainerType.__new__(ContainerType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.default_rate_in_stream = (_r_RateInStreamType(c.take())
                                if c.peek() == "DefaultRateInStream" else None)
    v.rate_in_stream_set = (_r_RateInStreamSetType(c.take())
                            if c.peek() == "RateInStreamSet" else None)
    v.binary_encoding = (_r_ContainerBinaryDataEncodingType(c.take())
                         if c.peek() == "BinaryEncoding" else None)
    c.end()
    return v


_A_ContextCalibratorListType = frozenset({})


def _r_ContextCalibratorListType(n: X.Node) -> ContextCalibratorListType:
    X.check_attrs(n, _A_ContextCalibratorListType)
    v = ContextCalibratorListType.__new__(ContextCalibratorListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextCalibrator':
        items.append(_r_ContextCalibratorType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextCalibrator', 1, len(items))
    v.context_calibrator = items
    c.end()
    return v


_A_ContextCalibratorType = frozenset({})


def _r_ContextCalibratorType(n: X.Node) -> ContextCalibratorType:
    X.check_attrs(n, _A_ContextCalibratorType)
    v = ContextCalibratorType.__new__(ContextCalibratorType)
    c = X.Cursor(n)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    if c.peek() != 'Calibrator':
        raise c.expected('Calibrator')
    v.calibrator = _r_CalibratorType(c.take())
    c.end()
    return v


_A_ContextMatchType = frozenset({})


def _r_ContextMatchType(n: X.Node) -> ContextMatchType:
    X.check_attrs(n, _A_ContextMatchType)
    v = ContextMatchType.__new__(ContextMatchType)
    c = X.Cursor(n)
    v.choice = _r_ContextMatchChoice(c)
    c.end()
    return v


_A_ContextSignificanceListType = frozenset({})


def _r_ContextSignificanceListType(n: X.Node) -> ContextSignificanceListType:
    X.check_attrs(n, _A_ContextSignificanceListType)
    v = ContextSignificanceListType.__new__(ContextSignificanceListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextSignificance':
        items.append(_r_ContextSignificanceType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextSignificance', 1, len(items))
    v.context_significance = items
    c.end()
    return v


_A_ContextSignificanceType = frozenset({})


def _r_ContextSignificanceType(n: X.Node) -> ContextSignificanceType:
    X.check_attrs(n, _A_ContextSignificanceType)
    v = ContextSignificanceType.__new__(ContextSignificanceType)
    c = X.Cursor(n)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    if c.peek() != 'Significance':
        raise c.expected('Significance')
    v.significance = _r_SignificanceType(c.take())
    c.end()
    return v


_A_CustomAlarmType = frozenset({'name', 'shortDescription'})


def _r_CustomAlarmType(n: X.Node) -> CustomAlarmType:
    X.check_attrs(n, _A_CustomAlarmType)
    v = CustomAlarmType.__new__(CustomAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'InputAlgorithm':
        raise c.expected('InputAlgorithm')
    v.input_algorithm = _r_InputAlgorithmType(c.take())
    c.end()
    return v


_A_CustomStreamType = frozenset({"shortDescription", "name", "bitRateInBPS",
                                 "pcmType", "inverted", "encodedStreamRef",
                                 "decodedStreamRef"})


def _r_CustomStreamType(n: X.Node) -> CustomStreamType:
    X.check_attrs(n, _A_CustomStreamType)
    v = CustomStreamType.__new__(CustomStreamType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.bit_rate_in_bps = X.opt_attr(n, 'bitRateInBPS', X.p_double)
    v.pcm_type = X.opt_attr(n, 'pcmType', _p_PCMType)
    v.inverted = X.opt_attr(n, 'inverted', X.p_bool)
    v.encoded_stream_ref = X.req_attr(n, 'encodedStreamRef', X.p_str)
    v.decoded_stream_ref = X.req_attr(n, 'decodedStreamRef', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'EncodingAlgorithm':
        raise c.expected('EncodingAlgorithm')
    v.encoding_algorithm = _r_InputAlgorithmType(c.take())
    if c.peek() != 'DecodingAlgorithm':
        raise c.expected('DecodingAlgorithm')
    v.decoding_algorithm = _r_InputOutputAlgorithmType(c.take())
    c.end()
    return v


_A_DataEncodingType = frozenset({'bitOrder', 'byteOrder'})


def _r_DataEncodingType(n: X.Node) -> DataEncodingType:
    X.check_attrs(n, _A_DataEncodingType)
    v = DataEncodingType.__new__(DataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    c.end()
    return v


_A_DescriptionType = frozenset({'shortDescription'})


def _r_DescriptionType(n: X.Node) -> DescriptionType:
    X.check_attrs(n, _A_DescriptionType)
    v = DescriptionType.__new__(DescriptionType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_DimensionListType = frozenset({})


def _r_DimensionListType(n: X.Node) -> DimensionListType:
    X.check_attrs(n, _A_DimensionListType)
    v = DimensionListType.__new__(DimensionListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Dimension':
        items.append(_r_DimensionType(c.take()))
    if len(items) < 1:
        raise c.too_few('Dimension', 1, len(items))
    v.dimension = items
    c.end()
    return v


_A_DimensionType = frozenset({})


def _r_DimensionType(n: X.Node) -> DimensionType:
    X.check_attrs(n, _A_DimensionType)
    v = DimensionType.__new__(DimensionType)
    c = X.Cursor(n)
    if c.peek() != 'StartingIndex':
        raise c.expected('StartingIndex')
    v.starting_index = _r_IntegerValueType(c.take())
    if c.peek() != 'EndingIndex':
        raise c.expected('EndingIndex')
    v.ending_index = _r_IntegerValueType(c.take())
    c.end()
    return v


_A_DiscreteLookupListType = frozenset({'defaultValue'})


def _r_DiscreteLookupListType(n: X.Node) -> DiscreteLookupListType:
    X.check_attrs(n, _A_DiscreteLookupListType)
    v = DiscreteLookupListType.__new__(DiscreteLookupListType)
    v.default_value = X.req_attr(n, 'defaultValue', X.p_i64)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'DiscreteLookup':
        items.append(_r_DiscreteLookupType(c.take()))
    if len(items) < 1:
        raise c.too_few('DiscreteLookup', 1, len(items))
    v.discrete_lookup = items
    c.end()
    return v


_A_DiscreteLookupType = frozenset({'value'})


def _r_DiscreteLookupType(n: X.Node) -> DiscreteLookupType:
    X.check_attrs(n, _A_DiscreteLookupType)
    v = DiscreteLookupType.__new__(DiscreteLookupType)
    v.value = X.req_attr(n, 'value', X.p_i64)
    c = X.Cursor(n)
    v.choice = _r_DiscreteLookupChoice(c)
    c.end()
    return v


_A_DynamicValueType = frozenset({})


def _r_DynamicValueType(n: X.Node) -> DynamicValueType:
    X.check_attrs(n, _A_DynamicValueType)
    v = DynamicValueType.__new__(DynamicValueType)
    c = X.Cursor(n)
    if c.peek() != 'ParameterInstanceRef':
        raise c.expected('ParameterInstanceRef')
    v.parameter_instance_ref = _r_ParameterInstanceRefType(c.take())
    v.linear_adjustment = (_r_LinearAdjustmentType(c.take())
                           if c.peek() == "LinearAdjustment" else None)
    c.end()
    return v


_A_EncodingType = frozenset({'units', 'scale', 'offset'})


def _r_EncodingType(n: X.Node) -> EncodingType:
    X.check_attrs(n, _A_EncodingType)
    v = EncodingType.__new__(EncodingType)
    v.units = X.opt_attr(n, 'units', _p_TimeUnitsType)
    v.scale = X.opt_attr(n, 'scale', X.p_double)
    v.offset = X.opt_attr(n, 'offset', X.p_double)
    c = X.Cursor(n)
    v.choice = _r_EncodingChoice(c)
    c.end()
    return v


_A_EntryListType = frozenset({})


def _r_EntryListType(n: X.Node) -> EntryListType:
    X.check_attrs(n, _A_EntryListType)
    v = EntryListType.__new__(EntryListType)
    c = X.Cursor(n)
    items = []
    while c.peek() in EntryListChoice.NAMES:
        items.append(_r_EntryListChoice(c))
    v.items = items
    c.end()
    return v


_A_EnumeratedArgumentType = frozenset({"shortDescription", "name", "baseType",
                                       "initialValue"})


def _r_EnumeratedArgumentType(n: X.Node) -> EnumeratedArgumentType:
    X.check_attrs(n, _A_EnumeratedArgumentType)
    v = EnumeratedArgumentType.__new__(EnumeratedArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_EnumeratedArgumentChoice(c)
                if c.peek() in EnumeratedArgumentChoice.NAMES else None)
    if c.peek() != 'EnumerationList':
        raise c.expected('EnumerationList')
    v.enumeration_list = _r_EnumerationListType(c.take())
    c.end()
    return v


_A_EnumeratedDataType = frozenset({"shortDescription", "name", "baseType",
                                   "initialValue"})


def _r_EnumeratedDataType(n: X.Node) -> EnumeratedDataType:
    X.check_attrs(n, _A_EnumeratedDataType)
    v = EnumeratedDataType.__new__(EnumeratedDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_EnumeratedDataChoice(c)
                if c.peek() in EnumeratedDataChoice.NAMES else None)
    if c.peek() != 'EnumerationList':
        raise c.expected('EnumerationList')
    v.enumeration_list = _r_EnumerationListType(c.take())
    c.end()
    return v


_A_EnumeratedParameterType = frozenset({"shortDescription", "name", "baseType",
                                        "initialValue"})


def _r_EnumeratedParameterType(n: X.Node) -> EnumeratedParameterType:
    X.check_attrs(n, _A_EnumeratedParameterType)
    v = EnumeratedParameterType.__new__(EnumeratedParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_EnumeratedParameterChoice(c)
                if c.peek() in EnumeratedParameterChoice.NAMES else None)
    if c.peek() != 'EnumerationList':
        raise c.expected('EnumerationList')
    v.enumeration_list = _r_EnumerationListType(c.take())
    v.default_alarm = (_r_EnumerationAlarmType(c.take())
                       if c.peek() == "DefaultAlarm" else None)
    v.context_alarm_list = (_r_EnumerationContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_EnumerationAlarmLevelType = frozenset({'alarmLevel', 'enumerationLabel'})


def _r_EnumerationAlarmLevelType(n: X.Node) -> EnumerationAlarmLevelType:
    X.check_attrs(n, _A_EnumerationAlarmLevelType)
    v = EnumerationAlarmLevelType.__new__(EnumerationAlarmLevelType)
    v.alarm_level = X.req_attr(n, 'alarmLevel', _p_ConcernLevelsType)
    v.enumeration_label = X.req_attr(n, 'enumerationLabel', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_EnumerationAlarmListType = frozenset({})


def _r_EnumerationAlarmListType(n: X.Node) -> EnumerationAlarmListType:
    X.check_attrs(n, _A_EnumerationAlarmListType)
    v = EnumerationAlarmListType.__new__(EnumerationAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'EnumerationAlarm':
        items.append(_r_EnumerationAlarmLevelType(c.take()))
    if len(items) < 1:
        raise c.too_few('EnumerationAlarm', 1, len(items))
    v.enumeration_alarm = items
    c.end()
    return v


_A_EnumerationAlarmType = frozenset({"name", "shortDescription",
                                     "minViolations", "minConformance",
                                     "disabled", "defaultAlarmLevel"})


def _r_EnumerationAlarmType(n: X.Node) -> EnumerationAlarmType:
    X.check_attrs(n, _A_EnumerationAlarmType)
    v = EnumerationAlarmType.__new__(EnumerationAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    v.default_alarm_level = X.opt_attr(n, "defaultAlarmLevel",
                                       _p_ConcernLevelsType)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_EnumerationAlarmChoice(c)
                if c.peek() in EnumerationAlarmChoice.NAMES else None)
    v.enumeration_alarm_list = (_r_EnumerationAlarmListType(c.take())
                                if c.peek() == "EnumerationAlarmList" else None)
    c.end()
    return v


_A_EnumerationContextAlarmListType = frozenset({})


def _r_EnumerationContextAlarmListType(
        n: X.Node) -> EnumerationContextAlarmListType:
    X.check_attrs(n, _A_EnumerationContextAlarmListType)
    v = EnumerationContextAlarmListType.__new__(EnumerationContextAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextAlarm':
        items.append(_r_EnumerationContextAlarmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextAlarm', 1, len(items))
    v.context_alarm = items
    c.end()
    return v


_A_EnumerationContextAlarmType = frozenset({"name", "shortDescription",
                                            "minViolations", "minConformance",
                                            "disabled", "defaultAlarmLevel"})


def _r_EnumerationContextAlarmType(n: X.Node) -> EnumerationContextAlarmType:
    X.check_attrs(n, _A_EnumerationContextAlarmType)
    v = EnumerationContextAlarmType.__new__(EnumerationContextAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    v.default_alarm_level = X.opt_attr(n, "defaultAlarmLevel",
                                       _p_ConcernLevelsType)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_EnumerationContextAlarmChoice(c)
                if c.peek() in EnumerationContextAlarmChoice.NAMES else None)
    v.enumeration_alarm_list = (_r_EnumerationAlarmListType(c.take())
                                if c.peek() == "EnumerationAlarmList" else None)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    c.end()
    return v


_A_EnumerationListType = frozenset({})


def _r_EnumerationListType(n: X.Node) -> EnumerationListType:
    X.check_attrs(n, _A_EnumerationListType)
    v = EnumerationListType.__new__(EnumerationListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Enumeration':
        items.append(_r_ValueEnumerationType(c.take()))
    if len(items) < 1:
        raise c.too_few('Enumeration', 1, len(items))
    v.enumeration = items
    c.end()
    return v


_A_ErrorDetectCorrectType = frozenset({})


def _r_ErrorDetectCorrectType(n: X.Node) -> ErrorDetectCorrectType:
    X.check_attrs(n, _A_ErrorDetectCorrectType)
    v = ErrorDetectCorrectType.__new__(ErrorDetectCorrectType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ErrorDetectCorrectChoice.NAMES:
        items.append(_r_ErrorDetectCorrectChoice(c))
    if len(items) < 1:
        raise c.too_few('Checksum|CRC|XOR|Parity', 1, len(items))
    v.items = items
    c.end()
    return v


_A_ExecutionVerifierType = frozenset({'shortDescription', 'name'})


def _r_ExecutionVerifierType(n: X.Node) -> ExecutionVerifierType:
    X.check_attrs(n, _A_ExecutionVerifierType)
    v = ExecutionVerifierType.__new__(ExecutionVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_ExecutionVerifierChoice1(c)
    v.choice2 = _r_ExecutionVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    v.percent_complete = (_r_PercentCompleteType(c.take())
                          if c.peek() == "PercentComplete" else None)
    c.end()
    return v


_A_ExternalAlgorithmSetType = frozenset({})


def _r_ExternalAlgorithmSetType(n: X.Node) -> ExternalAlgorithmSetType:
    X.check_attrs(n, _A_ExternalAlgorithmSetType)
    v = ExternalAlgorithmSetType.__new__(ExternalAlgorithmSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ExternalAlgorithm':
        items.append(_r_ExternalAlgorithmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ExternalAlgorithm', 1, len(items))
    v.external_algorithm = items
    c.end()
    return v


_A_ExternalAlgorithmType = frozenset({"implementationName",
                                      "algorithmLocation"})


def _r_ExternalAlgorithmType(n: X.Node) -> ExternalAlgorithmType:
    X.check_attrs(n, _A_ExternalAlgorithmType)
    v = ExternalAlgorithmType.__new__(ExternalAlgorithmType)
    v.implementation_name = X.req_attr(n, 'implementationName', X.p_str)
    v.algorithm_location = X.req_attr(n, 'algorithmLocation', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_FailedVerifierType = frozenset({'shortDescription', 'name'})


def _r_FailedVerifierType(n: X.Node) -> FailedVerifierType:
    X.check_attrs(n, _A_FailedVerifierType)
    v = FailedVerifierType.__new__(FailedVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_FailedVerifierChoice1(c)
    v.choice2 = _r_FailedVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    v.return_parm_ref = (_r_ParameterRefType(c.take())
                         if c.peek() == "ReturnParmRef" else None)
    c.end()
    return v


_A_FixedFrameStreamType = frozenset({"shortDescription", "name", "bitRateInBPS",
                                     "pcmType", "inverted",
                                     "syncApertureInBits", "frameLengthInBits"})


def _r_FixedFrameStreamType(n: X.Node) -> FixedFrameStreamType:
    X.check_attrs(n, _A_FixedFrameStreamType)
    v = FixedFrameStreamType.__new__(FixedFrameStreamType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.bit_rate_in_bps = X.opt_attr(n, 'bitRateInBPS', X.p_double)
    v.pcm_type = X.opt_attr(n, 'pcmType', _p_PCMType)
    v.inverted = X.opt_attr(n, 'inverted', X.p_bool)
    v.sync_aperture_in_bits = X.opt_attr(n, 'syncApertureInBits', X.p_i64)
    v.frame_length_in_bits = X.req_attr(n, 'frameLengthInBits', X.p_i64)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = _r_FixedFrameStreamChoice(c)
    v.stream_ref = (_r_StreamRefType(c.take()) if c.peek() == "StreamRef"
                    else None)
    if c.peek() != 'SyncStrategy':
        raise c.expected('SyncStrategy')
    v.sync_strategy = _r_FixedFrameSyncStrategyType(c.take())
    c.end()
    return v


_A_FixedFrameSyncStrategyType = frozenset({"verifyToLockGoodFrames",
                                           "checkToLockGoodFrames",
                                           "maxBitErrorsInSyncPattern"})


def _r_FixedFrameSyncStrategyType(n: X.Node) -> FixedFrameSyncStrategyType:
    X.check_attrs(n, _A_FixedFrameSyncStrategyType)
    v = FixedFrameSyncStrategyType.__new__(FixedFrameSyncStrategyType)
    v.verify_to_lock_good_frames = X.opt_attr(n, "verifyToLockGoodFrames",
                                              X.p_i64)
    v.check_to_lock_good_frames = X.opt_attr(n, "checkToLockGoodFrames",
                                             X.p_i64)
    v.max_bit_errors_in_sync_pattern = X.opt_attr(
        n, "maxBitErrorsInSyncPattern", X.p_i64)
    c = X.Cursor(n)
    v.auto_invert = (_r_AutoInvertType(c.take()) if c.peek() == "AutoInvert"
                     else None)
    if c.peek() != 'SyncPattern':
        raise c.expected('SyncPattern')
    v.sync_pattern = _r_SyncPatternType(c.take())
    c.end()
    return v


_A_FlagType = frozenset({'flagSizeInBits', 'flagBitType'})


def _r_FlagType(n: X.Node) -> FlagType:
    X.check_attrs(n, _A_FlagType)
    v = FlagType.__new__(FlagType)
    v.flag_size_in_bits = X.opt_attr(n, 'flagSizeInBits', X.p_i64)
    v.flag_bit_type = X.opt_attr(n, 'flagBitType', _p_FlagBitType)
    c = X.Cursor(n)
    c.end()
    return v


_A_FloatArgumentType = frozenset({"shortDescription", "name", "baseType",
                                  "initialValue", "sizeInBits"})


def _r_FloatArgumentType(n: X.Node) -> FloatArgumentType:
    X.check_attrs(n, _A_FloatArgumentType)
    v = FloatArgumentType.__new__(FloatArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_double)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', _p_FloatSizeInBitsType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_FloatArgumentChoice(c)
                if c.peek() in FloatArgumentChoice.NAMES else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    v.valid_range_set = (_r_ValidFloatRangeSetType(c.take())
                         if c.peek() == "ValidRangeSet" else None)
    c.end()
    return v


_A_FloatDataEncodingType = frozenset({"bitOrder", "byteOrder", "encoding",
                                      "sizeInBits", "changeThreshold"})


def _r_FloatDataEncodingType(n: X.Node) -> FloatDataEncodingType:
    X.check_attrs(n, _A_FloatDataEncodingType)
    v = FloatDataEncodingType.__new__(FloatDataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    v.encoding = X.opt_attr(n, 'encoding', _p_FloatEncodingType)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', _p_FloatEncodingSizeInBitsType)
    v.change_threshold = X.opt_attr(n, 'changeThreshold', X.p_double)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    v.default_calibrator = (_r_CalibratorType(c.take())
                            if c.peek() == "DefaultCalibrator" else None)
    v.context_calibrator_list = (_r_ContextCalibratorListType(c.take())
                                 if c.peek() == "ContextCalibratorList"
                                 else None)
    c.end()
    return v


_A_FloatDataType = frozenset({"shortDescription", "name", "baseType",
                              "initialValue", "sizeInBits"})


def _r_FloatDataType(n: X.Node) -> FloatDataType:
    X.check_attrs(n, _A_FloatDataType)
    v = FloatDataType.__new__(FloatDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_double)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', _p_FloatSizeInBitsType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_FloatDataChoice(c) if c.peek() in FloatDataChoice.NAMES
                else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    v.valid_range = (_r_FloatDataValidRangeType(c.take())
                     if c.peek() == "ValidRange" else None)
    c.end()
    return v


_A_FloatDataValidRangeType = frozenset({"minInclusive", "minExclusive",
                                        "maxInclusive", "maxExclusive",
                                        "validRangeAppliesToCalibrated"})


def _r_FloatDataValidRangeType(n: X.Node) -> FloatDataValidRangeType:
    X.check_attrs(n, _A_FloatDataValidRangeType)
    v = FloatDataValidRangeType.__new__(FloatDataValidRangeType)
    v.min_inclusive = X.opt_attr(n, 'minInclusive', X.p_double)
    v.min_exclusive = X.opt_attr(n, 'minExclusive', X.p_double)
    v.max_inclusive = X.opt_attr(n, 'maxInclusive', X.p_double)
    v.max_exclusive = X.opt_attr(n, 'maxExclusive', X.p_double)
    v.valid_range_applies_to_calibrated = X.opt_attr(
        n, "validRangeAppliesToCalibrated", X.p_bool)
    c = X.Cursor(n)
    c.end()
    return v


_A_FloatParameterType = frozenset({"shortDescription", "name", "baseType",
                                   "initialValue", "sizeInBits"})


def _r_FloatParameterType(n: X.Node) -> FloatParameterType:
    X.check_attrs(n, _A_FloatParameterType)
    v = FloatParameterType.__new__(FloatParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_double)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', _p_FloatSizeInBitsType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_FloatParameterChoice(c)
                if c.peek() in FloatParameterChoice.NAMES else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    v.valid_range = (_r_FloatDataValidRangeType(c.take())
                     if c.peek() == "ValidRange" else None)
    v.default_alarm = (_r_NumericAlarmType(c.take())
                       if c.peek() == "DefaultAlarm" else None)
    v.context_alarm_list = (_r_NumericContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_FloatRangeType = frozenset({"minInclusive", "minExclusive", "maxInclusive",
                               "maxExclusive"})


def _r_FloatRangeType(n: X.Node) -> FloatRangeType:
    X.check_attrs(n, _A_FloatRangeType)
    v = FloatRangeType.__new__(FloatRangeType)
    v.min_inclusive = X.opt_attr(n, 'minInclusive', X.p_double)
    v.min_exclusive = X.opt_attr(n, 'minExclusive', X.p_double)
    v.max_inclusive = X.opt_attr(n, 'maxInclusive', X.p_double)
    v.max_exclusive = X.opt_attr(n, 'maxExclusive', X.p_double)
    c = X.Cursor(n)
    c.end()
    return v


_A_FrameStreamType = frozenset({"shortDescription", "name", "bitRateInBPS",
                                "pcmType", "inverted"})


def _r_FrameStreamType(n: X.Node) -> FrameStreamType:
    X.check_attrs(n, _A_FrameStreamType)
    v = FrameStreamType.__new__(FrameStreamType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.bit_rate_in_bps = X.opt_attr(n, 'bitRateInBPS', X.p_double)
    v.pcm_type = X.opt_attr(n, 'pcmType', _p_PCMType)
    v.inverted = X.opt_attr(n, 'inverted', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = _r_FrameStreamChoice(c)
    v.stream_ref = (_r_StreamRefType(c.take()) if c.peek() == "StreamRef"
                    else None)
    c.end()
    return v


_A_HeaderType = frozenset({"version", "date", "classification",
                           "classificationInstructions", "validationStatus"})


def _r_HeaderType(n: X.Node) -> HeaderType:
    X.check_attrs(n, _A_HeaderType)
    v = HeaderType.__new__(HeaderType)
    v.version = X.opt_attr(n, 'version', X.p_str)
    v.date = X.opt_attr(n, 'date', X.p_str)
    v.classification = X.opt_attr(n, 'classification', X.p_str)
    v.classification_instructions = X.opt_attr(n, "classificationInstructions",
                                               X.p_str)
    v.validation_status = X.req_attr(n, "validationStatus",
                                     _p_ValidationStatusType)
    c = X.Cursor(n)
    v.author_set = (_r_AuthorSetType(c.take()) if c.peek() == "AuthorSet"
                    else None)
    v.note_set = _r_NoteSetType(c.take()) if c.peek() == 'NoteSet' else None
    v.history_set = (_r_HistorySetType(c.take()) if c.peek() == "HistorySet"
                     else None)
    c.end()
    return v


_A_HistorySetType = frozenset({})


def _r_HistorySetType(n: X.Node) -> HistorySetType:
    X.check_attrs(n, _A_HistorySetType)
    v = HistorySetType.__new__(HistorySetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'History':
        items.append(X.leaf(c.take(), X.p_str))
    v.history = items
    c.end()
    return v


_A_IndirectParameterRefEntryType = frozenset({"shortDescription",
                                              "aliasNameSpace"})


def _r_IndirectParameterRefEntryType(
        n: X.Node) -> IndirectParameterRefEntryType:
    X.check_attrs(n, _A_IndirectParameterRefEntryType)
    v = IndirectParameterRefEntryType.__new__(IndirectParameterRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.alias_name_space = X.opt_attr(n, 'aliasNameSpace', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'ParameterInstance':
        raise c.expected('ParameterInstance')
    v.parameter_instance = _r_ParameterInstanceRefType(c.take())
    c.end()
    return v


_A_InputAlgorithmType = frozenset({'shortDescription', 'name'})


def _r_InputAlgorithmType(n: X.Node) -> InputAlgorithmType:
    X.check_attrs(n, _A_InputAlgorithmType)
    v = InputAlgorithmType.__new__(InputAlgorithmType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.algorithm_text = (_r_AlgorithmTextType(c.take())
                        if c.peek() == "AlgorithmText" else None)
    v.external_algorithm_set = (_r_ExternalAlgorithmSetType(c.take())
                                if c.peek() == "ExternalAlgorithmSet" else None)
    v.input_set = _r_InputSetType(c.take()) if c.peek() == 'InputSet' else None
    c.end()
    return v


_A_InputOutputAlgorithmType = frozenset({'shortDescription', 'name', 'thread'})


def _r_InputOutputAlgorithmType(n: X.Node) -> InputOutputAlgorithmType:
    X.check_attrs(n, _A_InputOutputAlgorithmType)
    v = InputOutputAlgorithmType.__new__(InputOutputAlgorithmType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.thread = X.opt_attr(n, 'thread', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.algorithm_text = (_r_AlgorithmTextType(c.take())
                        if c.peek() == "AlgorithmText" else None)
    v.external_algorithm_set = (_r_ExternalAlgorithmSetType(c.take())
                                if c.peek() == "ExternalAlgorithmSet" else None)
    v.input_set = _r_InputSetType(c.take()) if c.peek() == 'InputSet' else None
    v.output_set = (_r_OutputSetType(c.take()) if c.peek() == "OutputSet"
                    else None)
    c.end()
    return v


_A_InputOutputTriggerAlgorithmType = frozenset(
    {"shortDescription", "name", "thread", "triggerContainer", "priority"})


def _r_InputOutputTriggerAlgorithmType(
        n: X.Node) -> InputOutputTriggerAlgorithmType:
    X.check_attrs(n, _A_InputOutputTriggerAlgorithmType)
    v = InputOutputTriggerAlgorithmType.__new__(InputOutputTriggerAlgorithmType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.thread = X.opt_attr(n, 'thread', X.p_bool)
    v.trigger_container = X.opt_attr(n, 'triggerContainer', X.p_str)
    v.priority = X.opt_attr(n, 'priority', X.p_i32)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.algorithm_text = (_r_AlgorithmTextType(c.take())
                        if c.peek() == "AlgorithmText" else None)
    v.external_algorithm_set = (_r_ExternalAlgorithmSetType(c.take())
                                if c.peek() == "ExternalAlgorithmSet" else None)
    v.input_set = _r_InputSetType(c.take()) if c.peek() == 'InputSet' else None
    v.output_set = (_r_OutputSetType(c.take()) if c.peek() == "OutputSet"
                    else None)
    v.trigger_set = (_r_TriggerSetType(c.take()) if c.peek() == "TriggerSet"
                     else None)
    c.end()
    return v


_A_InputParameterInstanceRefType = frozenset(
    {"parameterRef", "instance", "useCalibratedValue", "inputName"})


def _r_InputParameterInstanceRefType(
        n: X.Node) -> InputParameterInstanceRefType:
    X.check_attrs(n, _A_InputParameterInstanceRefType)
    v = InputParameterInstanceRefType.__new__(InputParameterInstanceRefType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.instance = X.opt_attr(n, 'instance', X.p_i64)
    v.use_calibrated_value = X.opt_attr(n, 'useCalibratedValue', X.p_bool)
    v.input_name = X.opt_attr(n, 'inputName', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_InputSetType = frozenset({})


def _r_InputSetType(n: X.Node) -> InputSetType:
    X.check_attrs(n, _A_InputSetType)
    v = InputSetType.__new__(InputSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in InputSetChoice.NAMES:
        items.append(_r_InputSetChoice(c))
    if len(items) < 1:
        raise c.too_few('InputParameterInstanceRef|Constant', 1, len(items))
    v.items = items
    c.end()
    return v


_A_IntegerArgumentType = frozenset({"shortDescription", "name", "baseType",
                                    "initialValue", "sizeInBits", "signed"})


def _r_IntegerArgumentType(n: X.Node) -> IntegerArgumentType:
    X.check_attrs(n, _A_IntegerArgumentType)
    v = IntegerArgumentType.__new__(IntegerArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_i64)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', X.p_i64)
    v.signed = X.opt_attr(n, 'signed', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_IntegerArgumentChoice(c)
                if c.peek() in IntegerArgumentChoice.NAMES else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    v.valid_range_set = (_r_ValidIntegerRangeSetType(c.take())
                         if c.peek() == "ValidRangeSet" else None)
    c.end()
    return v


_A_IntegerDataEncodingType = frozenset({"bitOrder", "byteOrder", "encoding",
                                        "sizeInBits", "changeThreshold"})


def _r_IntegerDataEncodingType(n: X.Node) -> IntegerDataEncodingType:
    X.check_attrs(n, _A_IntegerDataEncodingType)
    v = IntegerDataEncodingType.__new__(IntegerDataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    v.encoding = X.opt_attr(n, 'encoding', _p_IntegerEncodingType)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', X.p_i64)
    v.change_threshold = X.opt_attr(n, 'changeThreshold', X.p_i64)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    v.default_calibrator = (_r_CalibratorType(c.take())
                            if c.peek() == "DefaultCalibrator" else None)
    v.context_calibrator_list = (_r_ContextCalibratorListType(c.take())
                                 if c.peek() == "ContextCalibratorList"
                                 else None)
    c.end()
    return v


_A_IntegerDataType = frozenset({"shortDescription", "name", "baseType",
                                "initialValue", "sizeInBits", "signed"})


def _r_IntegerDataType(n: X.Node) -> IntegerDataType:
    X.check_attrs(n, _A_IntegerDataType)
    v = IntegerDataType.__new__(IntegerDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_i64)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', X.p_i64)
    v.signed = X.opt_attr(n, 'signed', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_IntegerDataChoice(c) if c.peek() in IntegerDataChoice.NAMES
                else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    v.valid_range = (_r_IntegerDataValidRangeType(c.take())
                     if c.peek() == "ValidRange" else None)
    c.end()
    return v


_A_IntegerDataValidRangeType = frozenset({"minInclusive", "maxInclusive",
                                          "validRangeAppliesToCalibrated"})


def _r_IntegerDataValidRangeType(n: X.Node) -> IntegerDataValidRangeType:
    X.check_attrs(n, _A_IntegerDataValidRangeType)
    v = IntegerDataValidRangeType.__new__(IntegerDataValidRangeType)
    v.min_inclusive = X.opt_attr(n, 'minInclusive', X.p_i64)
    v.max_inclusive = X.opt_attr(n, 'maxInclusive', X.p_i64)
    v.valid_range_applies_to_calibrated = X.opt_attr(
        n, "validRangeAppliesToCalibrated", X.p_bool)
    c = X.Cursor(n)
    c.end()
    return v


_A_IntegerParameterType = frozenset({"shortDescription", "name", "baseType",
                                     "initialValue", "sizeInBits", "signed"})


def _r_IntegerParameterType(n: X.Node) -> IntegerParameterType:
    X.check_attrs(n, _A_IntegerParameterType)
    v = IntegerParameterType.__new__(IntegerParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_i64)
    v.size_in_bits = X.opt_attr(n, 'sizeInBits', X.p_i64)
    v.signed = X.opt_attr(n, 'signed', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_IntegerParameterChoice(c)
                if c.peek() in IntegerParameterChoice.NAMES else None)
    v.to_string = _r_ToStringType(c.take()) if c.peek() == 'ToString' else None
    v.valid_range = (_r_IntegerDataValidRangeType(c.take())
                     if c.peek() == "ValidRange" else None)
    v.default_alarm = (_r_NumericAlarmType(c.take())
                       if c.peek() == "DefaultAlarm" else None)
    v.context_alarm_list = (_r_NumericContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_IntegerRangeType = frozenset({'minInclusive', 'maxInclusive'})


def _r_IntegerRangeType(n: X.Node) -> IntegerRangeType:
    X.check_attrs(n, _A_IntegerRangeType)
    v = IntegerRangeType.__new__(IntegerRangeType)
    v.min_inclusive = X.opt_attr(n, 'minInclusive', X.p_i64)
    v.max_inclusive = X.opt_attr(n, 'maxInclusive', X.p_i64)
    c = X.Cursor(n)
    c.end()
    return v


_A_IntegerValueType = frozenset({})


def _r_IntegerValueType(n: X.Node) -> IntegerValueType:
    X.check_attrs(n, _A_IntegerValueType)
    v = IntegerValueType.__new__(IntegerValueType)
    c = X.Cursor(n)
    v.choice = _r_IntegerValueChoice(c)
    c.end()
    return v


_A_InterlockType = frozenset({"scopeToSpaceSystem", "verificationToWaitFor",
                              "verificationProgressPercentage", "suspendable"})


def _r_InterlockType(n: X.Node) -> InterlockType:
    X.check_attrs(n, _A_InterlockType)
    v = InterlockType.__new__(InterlockType)
    v.scope_to_space_system = X.opt_attr(n, 'scopeToSpaceSystem', X.p_str)
    v.verification_to_wait_for = X.opt_attr(n, "verificationToWaitFor",
                                            _p_VerifierEnumerationType)
    v.verification_progress_percentage = X.opt_attr(
        n, "verificationProgressPercentage", X.p_double)
    v.suspendable = X.opt_attr(n, 'suspendable', X.p_bool)
    c = X.Cursor(n)
    c.end()
    return v


_A_LeadingSizeType = frozenset({'sizeInBitsOfSizeTag'})


def _r_LeadingSizeType(n: X.Node) -> LeadingSizeType:
    X.check_attrs(n, _A_LeadingSizeType)
    v = LeadingSizeType.__new__(LeadingSizeType)
    v.size_in_bits_of_size_tag = X.opt_attr(n, 'sizeInBitsOfSizeTag', X.p_i64)
    c = X.Cursor(n)
    c.end()
    return v


_A_LinearAdjustmentType = frozenset({'slope', 'intercept'})


def _r_LinearAdjustmentType(n: X.Node) -> LinearAdjustmentType:
    X.check_attrs(n, _A_LinearAdjustmentType)
    v = LinearAdjustmentType.__new__(LinearAdjustmentType)
    v.slope = X.opt_attr(n, 'slope', X.p_double)
    v.intercept = X.opt_attr(n, 'intercept', X.p_double)
    c = X.Cursor(n)
    c.end()
    return v


_A_LocationInContainerInBitsType = frozenset({'referenceLocation'})


def _r_LocationInContainerInBitsType(
        n: X.Node) -> LocationInContainerInBitsType:
    X.check_attrs(n, _A_LocationInContainerInBitsType)
    v = LocationInContainerInBitsType.__new__(LocationInContainerInBitsType)
    v.reference_location = X.opt_attr(n, "referenceLocation",
                                      _p_ReferenceLocationType)
    c = X.Cursor(n)
    v.choice = _r_LocationInContainerInBitsChoice(c)
    c.end()
    return v


_A_MatchCriteriaType = frozenset({})


def _r_MatchCriteriaType(n: X.Node) -> MatchCriteriaType:
    X.check_attrs(n, _A_MatchCriteriaType)
    v = MatchCriteriaType.__new__(MatchCriteriaType)
    c = X.Cursor(n)
    v.choice = _r_MatchCriteriaChoice(c)
    c.end()
    return v


_A_MathAlgorithmType = frozenset({'shortDescription', 'name'})


def _r_MathAlgorithmType(n: X.Node) -> MathAlgorithmType:
    X.check_attrs(n, _A_MathAlgorithmType)
    v = MathAlgorithmType.__new__(MathAlgorithmType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'MathOperation':
        raise c.expected('MathOperation')
    v.math_operation = _r_TriggeredMathOperationType(c.take())
    c.end()
    return v


_A_MathOperationCalibratorType = frozenset({'name', 'shortDescription'})


def _r_MathOperationCalibratorType(n: X.Node) -> MathOperationCalibratorType:
    X.check_attrs(n, _A_MathOperationCalibratorType)
    v = MathOperationCalibratorType.__new__(MathOperationCalibratorType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() in MathOperationCalibratorChoice.NAMES:
        items.append(_r_MathOperationCalibratorChoice(c))
    if len(items) < 1:
        raise c.too_few("ValueOperand|ThisParameterOperand|Operator|"
                        "ParameterInstanceRefOperand", 1, len(items))
    v.items = items
    c.end()
    return v


_A_MathOperationType = frozenset({'name', 'shortDescription'})


def _r_MathOperationType(n: X.Node) -> MathOperationType:
    X.check_attrs(n, _A_MathOperationType)
    v = MathOperationType.__new__(MathOperationType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() in MathOperationChoice.NAMES:
        items.append(_r_MathOperationChoice(c))
    if len(items) < 1:
        raise c.too_few("ValueOperand|ThisParameterOperand|Operator|"
                        "ParameterInstanceRefOperand", 1, len(items))
    v.items = items
    c.end()
    return v


_A_MemberListType = frozenset({})


def _r_MemberListType(n: X.Node) -> MemberListType:
    X.check_attrs(n, _A_MemberListType)
    v = MemberListType.__new__(MemberListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Member':
        items.append(_r_MemberType(c.take()))
    if len(items) < 1:
        raise c.too_few('Member', 1, len(items))
    v.member = items
    c.end()
    return v


_A_MemberType = frozenset({"shortDescription", "name", "typeRef",
                           "initialValue"})


def _r_MemberType(n: X.Node) -> MemberType:
    X.check_attrs(n, _A_MemberType)
    v = MemberType.__new__(MemberType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.type_ref = X.req_attr(n, 'typeRef', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_MessageRefSetType = frozenset({})


def _r_MessageRefSetType(n: X.Node) -> MessageRefSetType:
    X.check_attrs(n, _A_MessageRefSetType)
    v = MessageRefSetType.__new__(MessageRefSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'MessageRef':
        items.append(_r_MessageRefType(c.take()))
    if len(items) < 1:
        raise c.too_few('MessageRef', 1, len(items))
    v.message_ref = items
    c.end()
    return v


_A_MessageRefType = frozenset({'messageRef'})


def _r_MessageRefType(n: X.Node) -> MessageRefType:
    X.check_attrs(n, _A_MessageRefType)
    v = MessageRefType.__new__(MessageRefType)
    v.message_ref = X.req_attr(n, 'messageRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_MessageSetType = frozenset({'shortDescription', 'name'})


def _r_MessageSetType(n: X.Node) -> MessageSetType:
    X.check_attrs(n, _A_MessageSetType)
    v = MessageSetType.__new__(MessageSetType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() == 'Message':
        items.append(_r_MessageType(c.take()))
    if len(items) < 1:
        raise c.too_few('Message', 1, len(items))
    v.message = items
    c.end()
    return v


_A_MessageType = frozenset({'shortDescription', 'name'})


def _r_MessageType(n: X.Node) -> MessageType:
    X.check_attrs(n, _A_MessageType)
    v = MessageType.__new__(MessageType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    if c.peek() != 'MatchCriteria':
        raise c.expected('MatchCriteria')
    v.match_criteria = _r_MatchCriteriaType(c.take())
    if c.peek() != 'ContainerRef':
        raise c.expected('ContainerRef')
    v.container_ref = _r_ContainerRefType(c.take())
    c.end()
    return v


_A_MetaCommandSetType = frozenset({})


def _r_MetaCommandSetType(n: X.Node) -> MetaCommandSetType:
    X.check_attrs(n, _A_MetaCommandSetType)
    v = MetaCommandSetType.__new__(MetaCommandSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in MetaCommandSetChoice.NAMES:
        items.append(_r_MetaCommandSetChoice(c))
    if len(items) < 1:
        raise c.too_few("MetaCommand|MetaCommandRef|BlockMetaCommand", 1,
                        len(items))
    v.items = items
    c.end()
    return v


_A_MetaCommandStepListType = frozenset({})


def _r_MetaCommandStepListType(n: X.Node) -> MetaCommandStepListType:
    X.check_attrs(n, _A_MetaCommandStepListType)
    v = MetaCommandStepListType.__new__(MetaCommandStepListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'MetaCommandStep':
        items.append(_r_MetaCommandStepType(c.take()))
    if len(items) < 1:
        raise c.too_few('MetaCommandStep', 1, len(items))
    v.meta_command_step = items
    c.end()
    return v


_A_MetaCommandStepType = frozenset({'metaCommandRef'})


def _r_MetaCommandStepType(n: X.Node) -> MetaCommandStepType:
    X.check_attrs(n, _A_MetaCommandStepType)
    v = MetaCommandStepType.__new__(MetaCommandStepType)
    v.meta_command_ref = X.req_attr(n, 'metaCommandRef', X.p_str)
    c = X.Cursor(n)
    v.argument_assignment_list = (_r_ArgumentAssignmentListType(c.take())
                                  if c.peek() == "ArgumentAssignmentList"
                                  else None)
    c.end()
    return v


_A_MetaCommandType = frozenset({'shortDescription', 'name', 'abstract'})


def _r_MetaCommandType(n: X.Node) -> MetaCommandType:
    X.check_attrs(n, _A_MetaCommandType)
    v = MetaCommandType.__new__(MetaCommandType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.abstract = X.opt_attr(n, 'abstract', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.base_meta_command = (_r_BaseMetaCommandType(c.take())
                           if c.peek() == "BaseMetaCommand" else None)
    v.system_name = (X.leaf(c.take(), X.p_str) if c.peek() == "SystemName"
                     else None)
    v.argument_list = (_r_ArgumentListType(c.take())
                       if c.peek() == "ArgumentList" else None)
    v.command_container = (_r_CommandContainerType(c.take())
                           if c.peek() == "CommandContainer" else None)
    v.transmission_constraint_list = _r_TransmissionConstraintListType(
        c.take()) if c.peek() == 'TransmissionConstraintList' else None
    v.default_significance = (_r_SignificanceType(c.take())
                              if c.peek() == "DefaultSignificance" else None)
    v.context_significance_list = (_r_ContextSignificanceListType(c.take())
                                   if c.peek() == "ContextSignificanceList"
                                   else None)
    v.interlock = (_r_InterlockType(c.take()) if c.peek() == "Interlock"
                   else None)
    v.verifier_set = (_r_VerifierSetType(c.take()) if c.peek() == "VerifierSet"
                      else None)
    v.parameter_to_set_list = (_r_ParameterToSetListType(c.take())
                               if c.peek() == "ParameterToSetList" else None)
    v.parameters_to_suspend_alarms_on_set = (
        _r_ParametersToSuspendAlarmsOnSetType(c.take())
        if c.peek() == "ParametersToSuspendAlarmsOnSet" else None)
    c.end()
    return v


_A_MultiRangeType = frozenset({"minInclusive", "minExclusive", "maxInclusive",
                               "maxExclusive", "rangeForm", "level"})


def _r_MultiRangeType(n: X.Node) -> MultiRangeType:
    X.check_attrs(n, _A_MultiRangeType)
    v = MultiRangeType.__new__(MultiRangeType)
    v.min_inclusive = X.opt_attr(n, 'minInclusive', X.p_double)
    v.min_exclusive = X.opt_attr(n, 'minExclusive', X.p_double)
    v.max_inclusive = X.opt_attr(n, 'maxInclusive', X.p_double)
    v.max_exclusive = X.opt_attr(n, 'maxExclusive', X.p_double)
    v.range_form = X.opt_attr(n, 'rangeForm', _p_RangeFormType)
    v.level = X.opt_attr(n, 'level', _p_ConcernLevelsType)
    c = X.Cursor(n)
    c.end()
    return v


_A_NameDescriptionType = frozenset({'shortDescription', 'name'})


def _r_NameDescriptionType(n: X.Node) -> NameDescriptionType:
    X.check_attrs(n, _A_NameDescriptionType)
    v = NameDescriptionType.__new__(NameDescriptionType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_NoteSetType = frozenset({})


def _r_NoteSetType(n: X.Node) -> NoteSetType:
    X.check_attrs(n, _A_NoteSetType)
    v = NoteSetType.__new__(NoteSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Note':
        items.append(X.leaf(c.take(), X.p_str))
    v.note = items
    c.end()
    return v


_A_NumberFormatType = frozenset(
    {"numberBase", "minimumFractionDigits", "maximumFractionDigits",
     "minimumIntegerDigits", "maximumIntegerDigits", "negativeSuffix",
     "positiveSuffix", "negativePrefix", "positivePrefix",
     "showThousandsGrouping", "notation"})


def _r_NumberFormatType(n: X.Node) -> NumberFormatType:
    X.check_attrs(n, _A_NumberFormatType)
    v = NumberFormatType.__new__(NumberFormatType)
    v.number_base = X.opt_attr(n, 'numberBase', _p_RadixType)
    v.minimum_fraction_digits = X.opt_attr(n, 'minimumFractionDigits', X.p_i64)
    v.maximum_fraction_digits = X.opt_attr(n, 'maximumFractionDigits', X.p_i64)
    v.minimum_integer_digits = X.opt_attr(n, 'minimumIntegerDigits', X.p_i64)
    v.maximum_integer_digits = X.opt_attr(n, 'maximumIntegerDigits', X.p_i64)
    v.negative_suffix = X.opt_attr(n, 'negativeSuffix', X.p_str)
    v.positive_suffix = X.opt_attr(n, 'positiveSuffix', X.p_str)
    v.negative_prefix = X.opt_attr(n, 'negativePrefix', X.p_str)
    v.positive_prefix = X.opt_attr(n, 'positivePrefix', X.p_str)
    v.show_thousands_grouping = X.opt_attr(n, 'showThousandsGrouping', X.p_bool)
    v.notation = X.opt_attr(n, 'notation', _p_FloatingPointNotationType)
    c = X.Cursor(n)
    c.end()
    return v


_A_NumericAlarmType = frozenset({"name", "shortDescription", "minViolations",
                                 "minConformance", "disabled"})


def _r_NumericAlarmType(n: X.Node) -> NumericAlarmType:
    X.check_attrs(n, _A_NumericAlarmType)
    v = NumericAlarmType.__new__(NumericAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_NumericAlarmChoice(c) if c.peek() in NumericAlarmChoice.NAMES
                else None)
    v.static_alarm_ranges = (_r_AlarmRangesType(c.take())
                             if c.peek() == "StaticAlarmRanges" else None)
    v.change_alarm_ranges = (_r_ChangeAlarmRangesType(c.take())
                             if c.peek() == "ChangeAlarmRanges" else None)
    v.alarm_multi_ranges = (_r_AlarmMultiRangesType(c.take())
                            if c.peek() == "AlarmMultiRanges" else None)
    c.end()
    return v


_A_NumericContextAlarmListType = frozenset({})


def _r_NumericContextAlarmListType(n: X.Node) -> NumericContextAlarmListType:
    X.check_attrs(n, _A_NumericContextAlarmListType)
    v = NumericContextAlarmListType.__new__(NumericContextAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextAlarm':
        items.append(_r_NumericContextAlarmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextAlarm', 1, len(items))
    v.context_alarm = items
    c.end()
    return v


_A_NumericContextAlarmType = frozenset(
    {"name", "shortDescription", "minViolations", "minConformance", "disabled"})


def _r_NumericContextAlarmType(n: X.Node) -> NumericContextAlarmType:
    X.check_attrs(n, _A_NumericContextAlarmType)
    v = NumericContextAlarmType.__new__(NumericContextAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_NumericContextAlarmChoice(c)
                if c.peek() in NumericContextAlarmChoice.NAMES else None)
    v.static_alarm_ranges = (_r_AlarmRangesType(c.take())
                             if c.peek() == "StaticAlarmRanges" else None)
    v.change_alarm_ranges = (_r_ChangeAlarmRangesType(c.take())
                             if c.peek() == "ChangeAlarmRanges" else None)
    v.alarm_multi_ranges = (_r_AlarmMultiRangesType(c.take())
                            if c.peek() == "AlarmMultiRanges" else None)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    c.end()
    return v


_A_ORedConditionsType = frozenset({})


def _r_ORedConditionsType(n: X.Node) -> ORedConditionsType:
    X.check_attrs(n, _A_ORedConditionsType)
    v = ORedConditionsType.__new__(ORedConditionsType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ORedConditionsChoice.NAMES:
        items.append(_r_ORedConditionsChoice(c))
    if len(items) < 2:
        raise c.too_few('Condition|ANDedConditions', 2, len(items))
    v.items = items
    c.end()
    return v


_A_OnContainerUpdateTriggerType = frozenset({'containerRef'})


def _r_OnContainerUpdateTriggerType(n: X.Node) -> OnContainerUpdateTriggerType:
    X.check_attrs(n, _A_OnContainerUpdateTriggerType)
    v = OnContainerUpdateTriggerType.__new__(OnContainerUpdateTriggerType)
    v.container_ref = X.req_attr(n, 'containerRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_OnParameterUpdateTriggerType = frozenset({'parameterRef'})


def _r_OnParameterUpdateTriggerType(n: X.Node) -> OnParameterUpdateTriggerType:
    X.check_attrs(n, _A_OnParameterUpdateTriggerType)
    v = OnParameterUpdateTriggerType.__new__(OnParameterUpdateTriggerType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_OnPeriodicRateTriggerType = frozenset({'fireRateInSeconds'})


def _r_OnPeriodicRateTriggerType(n: X.Node) -> OnPeriodicRateTriggerType:
    X.check_attrs(n, _A_OnPeriodicRateTriggerType)
    v = OnPeriodicRateTriggerType.__new__(OnPeriodicRateTriggerType)
    v.fire_rate_in_seconds = X.req_attr(n, 'fireRateInSeconds', X.p_double)
    c = X.Cursor(n)
    c.end()
    return v


_A_OptionalNameDescriptionType = frozenset({'shortDescription', 'name'})


def _r_OptionalNameDescriptionType(n: X.Node) -> OptionalNameDescriptionType:
    X.check_attrs(n, _A_OptionalNameDescriptionType)
    v = OptionalNameDescriptionType.__new__(OptionalNameDescriptionType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_OutputParameterRefType = frozenset({'parameterRef', 'outputName'})


def _r_OutputParameterRefType(n: X.Node) -> OutputParameterRefType:
    X.check_attrs(n, _A_OutputParameterRefType)
    v = OutputParameterRefType.__new__(OutputParameterRefType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.output_name = X.opt_attr(n, 'outputName', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_OutputSetType = frozenset({})


def _r_OutputSetType(n: X.Node) -> OutputSetType:
    X.check_attrs(n, _A_OutputSetType)
    v = OutputSetType.__new__(OutputSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'OutputParameterRef':
        items.append(_r_OutputParameterRefType(c.take()))
    if len(items) < 1:
        raise c.too_few('OutputParameterRef', 1, len(items))
    v.output_parameter_ref = items
    c.end()
    return v


_A_PCMStreamType = frozenset({"shortDescription", "name", "bitRateInBPS",
                              "pcmType", "inverted"})


def _r_PCMStreamType(n: X.Node) -> PCMStreamType:
    X.check_attrs(n, _A_PCMStreamType)
    v = PCMStreamType.__new__(PCMStreamType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.bit_rate_in_bps = X.opt_attr(n, 'bitRateInBPS', X.p_double)
    v.pcm_type = X.opt_attr(n, 'pcmType', _p_PCMType)
    v.inverted = X.opt_attr(n, 'inverted', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ParameterInstanceRefType = frozenset({"parameterRef", "instance",
                                         "useCalibratedValue"})


def _r_ParameterInstanceRefType(n: X.Node) -> ParameterInstanceRefType:
    X.check_attrs(n, _A_ParameterInstanceRefType)
    v = ParameterInstanceRefType.__new__(ParameterInstanceRefType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.instance = X.opt_attr(n, 'instance', X.p_i64)
    v.use_calibrated_value = X.opt_attr(n, 'useCalibratedValue', X.p_bool)
    c = X.Cursor(n)
    c.end()
    return v


_A_ParameterPropertiesType = frozenset({"dataSource", "readOnly",
                                        "persistence"})


def _r_ParameterPropertiesType(n: X.Node) -> ParameterPropertiesType:
    X.check_attrs(n, _A_ParameterPropertiesType)
    v = ParameterPropertiesType.__new__(ParameterPropertiesType)
    v.data_source = X.opt_attr(n, 'dataSource', _p_TelemetryDataSourceType)
    v.read_only = X.opt_attr(n, 'readOnly', X.p_bool)
    v.persistence = X.opt_attr(n, 'persistence', X.p_bool)
    c = X.Cursor(n)
    v.system_name = (X.leaf(c.take(), X.p_str) if c.peek() == "SystemName"
                     else None)
    v.validity_condition = (_r_MatchCriteriaType(c.take())
                            if c.peek() == "ValidityCondition" else None)
    v.physical_address_set = (_r_PhysicalAddressSetType(c.take())
                              if c.peek() == "PhysicalAddressSet" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    c.end()
    return v


_A_ParameterRefEntryType = frozenset({'shortDescription', 'parameterRef'})


def _r_ParameterRefEntryType(n: X.Node) -> ParameterRefEntryType:
    X.check_attrs(n, _A_ParameterRefEntryType)
    v = ParameterRefEntryType.__new__(ParameterRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ParameterRefType = frozenset({'parameterRef'})


def _r_ParameterRefType(n: X.Node) -> ParameterRefType:
    X.check_attrs(n, _A_ParameterRefType)
    v = ParameterRefType.__new__(ParameterRefType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_ParameterSegmentRefEntryType = frozenset({"shortDescription", "parameterRef",
                                             "order", "sizeInBits"})


def _r_ParameterSegmentRefEntryType(n: X.Node) -> ParameterSegmentRefEntryType:
    X.check_attrs(n, _A_ParameterSegmentRefEntryType)
    v = ParameterSegmentRefEntryType.__new__(ParameterSegmentRefEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ParameterSetType = frozenset({})


def _r_ParameterSetType(n: X.Node) -> ParameterSetType:
    X.check_attrs(n, _A_ParameterSetType)
    v = ParameterSetType.__new__(ParameterSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ParameterSetChoice.NAMES:
        items.append(_r_ParameterSetChoice(c))
    if len(items) < 1:
        raise c.too_few('Parameter|ParameterRef', 1, len(items))
    v.items = items
    c.end()
    return v


_A_ParameterToSetListType = frozenset({})


def _r_ParameterToSetListType(n: X.Node) -> ParameterToSetListType:
    X.check_attrs(n, _A_ParameterToSetListType)
    v = ParameterToSetListType.__new__(ParameterToSetListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ParameterToSet':
        items.append(_r_ParameterToSetType(c.take()))
    if len(items) < 1:
        raise c.too_few('ParameterToSet', 1, len(items))
    v.parameter_to_set = items
    c.end()
    return v


_A_ParameterToSetType = frozenset({'parameterRef', 'setOnVerification'})


def _r_ParameterToSetType(n: X.Node) -> ParameterToSetType:
    X.check_attrs(n, _A_ParameterToSetType)
    v = ParameterToSetType.__new__(ParameterToSetType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.set_on_verification = X.opt_attr(n, "setOnVerification",
                                       _p_VerifierEnumerationType)
    c = X.Cursor(n)
    v.choice = _r_ParameterToSetChoice(c)
    c.end()
    return v


_A_ParameterToSuspendAlarmsOnType = frozenset({"parameterRef", "suspenseTime",
                                               "verifierToTriggerOn"})


def _r_ParameterToSuspendAlarmsOnType(
        n: X.Node) -> ParameterToSuspendAlarmsOnType:
    X.check_attrs(n, _A_ParameterToSuspendAlarmsOnType)
    v = ParameterToSuspendAlarmsOnType.__new__(ParameterToSuspendAlarmsOnType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.suspense_time = X.req_attr(n, 'suspenseTime', X.p_str)
    v.verifier_to_trigger_on = X.opt_attr(n, "verifierToTriggerOn",
                                          _p_VerifierEnumerationType)
    c = X.Cursor(n)
    c.end()
    return v


_A_ParameterType = frozenset({"shortDescription", "name", "parameterTypeRef",
                              "initialValue"})


def _r_ParameterType(n: X.Node) -> ParameterType:
    X.check_attrs(n, _A_ParameterType)
    v = ParameterType.__new__(ParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.parameter_type_ref = X.req_attr(n, 'parameterTypeRef', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.parameter_properties = (_r_ParameterPropertiesType(c.take())
                              if c.peek() == "ParameterProperties" else None)
    c.end()
    return v


_A_ParameterTypeSetType = frozenset({})


def _r_ParameterTypeSetType(n: X.Node) -> ParameterTypeSetType:
    X.check_attrs(n, _A_ParameterTypeSetType)
    v = ParameterTypeSetType.__new__(ParameterTypeSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in ParameterTypeSetChoice.NAMES:
        items.append(_r_ParameterTypeSetChoice(c))
    if len(items) < 1:
        raise c.too_few(
            "StringParameterType|EnumeratedParameterType|IntegerParameterType|"
            "BinaryParameterType|FloatParameterType|BooleanParameterType|"
            "RelativeTimeParameterType|AbsoluteTimeParameterType|"
            "ArrayParameterType|AggregateParameterType", 1, len(items))
    v.items = items
    c.end()
    return v


_A_ParameterValueChangeType = frozenset({})


def _r_ParameterValueChangeType(n: X.Node) -> ParameterValueChangeType:
    X.check_attrs(n, _A_ParameterValueChangeType)
    v = ParameterValueChangeType.__new__(ParameterValueChangeType)
    c = X.Cursor(n)
    if c.peek() != 'ParameterRef':
        raise c.expected('ParameterRef')
    v.parameter_ref = _r_ParameterRefType(c.take())
    if c.peek() != 'Change':
        raise c.expected('Change')
    v.change = _r_ChangeValueType(c.take())
    c.end()
    return v


_A_ParametersToSuspendAlarmsOnSetType = frozenset({})


def _r_ParametersToSuspendAlarmsOnSetType(
        n: X.Node) -> ParametersToSuspendAlarmsOnSetType:
    X.check_attrs(n, _A_ParametersToSuspendAlarmsOnSetType)
    v = ParametersToSuspendAlarmsOnSetType.__new__(
        ParametersToSuspendAlarmsOnSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ParameterToSuspendAlarmsOn':
        items.append(_r_ParameterToSuspendAlarmsOnType(c.take()))
    if len(items) < 1:
        raise c.too_few('ParameterToSuspendAlarmsOn', 1, len(items))
    v.parameter_to_suspend_alarms_on = items
    c.end()
    return v


_A_ParityType = frozenset({"type", "bitsFromReference", "reference",
                           "parameterRef"})


def _r_ParityType(n: X.Node) -> ParityType:
    X.check_attrs(n, _A_ParityType)
    v = ParityType.__new__(ParityType)
    v.type = X.req_attr(n, 'type', _p_ParityFormType)
    v.bits_from_reference = X.opt_attr(n, 'bitsFromReference', X.p_i64)
    v.reference = X.opt_attr(n, 'reference', _p_ReferencePointType)
    v.parameter_ref = X.opt_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_PercentCompleteType = frozenset({})


def _r_PercentCompleteType(n: X.Node) -> PercentCompleteType:
    X.check_attrs(n, _A_PercentCompleteType)
    v = PercentCompleteType.__new__(PercentCompleteType)
    c = X.Cursor(n)
    v.choice = _r_PercentCompleteChoice(c)
    c.end()
    return v


_A_PhysicalAddressSetType = frozenset({})


def _r_PhysicalAddressSetType(n: X.Node) -> PhysicalAddressSetType:
    X.check_attrs(n, _A_PhysicalAddressSetType)
    v = PhysicalAddressSetType.__new__(PhysicalAddressSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'PhysicalAddress':
        items.append(_r_PhysicalAddressType(c.take()))
    v.physical_address = items
    c.end()
    return v


_A_PhysicalAddressType = frozenset({'sourceName', 'sourceAddress'})


def _r_PhysicalAddressType(n: X.Node) -> PhysicalAddressType:
    X.check_attrs(n, _A_PhysicalAddressType)
    v = PhysicalAddressType.__new__(PhysicalAddressType)
    v.source_name = X.opt_attr(n, 'sourceName', X.p_str)
    v.source_address = X.opt_attr(n, 'sourceAddress', X.p_str)
    c = X.Cursor(n)
    v.sub_address = (_r_PhysicalAddressType(c.take())
                     if c.peek() == "SubAddress" else None)
    c.end()
    return v


_A_PolynomialCalibratorType = frozenset({'name', 'shortDescription'})


def _r_PolynomialCalibratorType(n: X.Node) -> PolynomialCalibratorType:
    X.check_attrs(n, _A_PolynomialCalibratorType)
    v = PolynomialCalibratorType.__new__(PolynomialCalibratorType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() == 'Term':
        items.append(_r_TermType(c.take()))
    if len(items) < 1:
        raise c.too_few('Term', 1, len(items))
    v.term = items
    c.end()
    return v


_A_QueuedVerifierType = frozenset({'shortDescription', 'name'})


def _r_QueuedVerifierType(n: X.Node) -> QueuedVerifierType:
    X.check_attrs(n, _A_QueuedVerifierType)
    v = QueuedVerifierType.__new__(QueuedVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_QueuedVerifierChoice1(c)
    v.choice2 = _r_QueuedVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_RateInStreamSetType = frozenset({})


def _r_RateInStreamSetType(n: X.Node) -> RateInStreamSetType:
    X.check_attrs(n, _A_RateInStreamSetType)
    v = RateInStreamSetType.__new__(RateInStreamSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'RateInStream':
        items.append(_r_RateInStreamWithStreamNameType(c.take()))
    if len(items) < 1:
        raise c.too_few('RateInStream', 1, len(items))
    v.rate_in_stream = items
    c.end()
    return v


_A_RateInStreamType = frozenset({'basis', 'minimumValue', 'maximumValue'})


def _r_RateInStreamType(n: X.Node) -> RateInStreamType:
    X.check_attrs(n, _A_RateInStreamType)
    v = RateInStreamType.__new__(RateInStreamType)
    v.basis = X.opt_attr(n, 'basis', _p_BasisType)
    v.minimum_value = X.opt_attr(n, 'minimumValue', X.p_double)
    v.maximum_value = X.opt_attr(n, 'maximumValue', X.p_double)
    c = X.Cursor(n)
    c.end()
    return v


_A_RateInStreamWithStreamNameType = frozenset({"basis", "minimumValue",
                                               "maximumValue", "streamRef"})


def _r_RateInStreamWithStreamNameType(
        n: X.Node) -> RateInStreamWithStreamNameType:
    X.check_attrs(n, _A_RateInStreamWithStreamNameType)
    v = RateInStreamWithStreamNameType.__new__(RateInStreamWithStreamNameType)
    v.basis = X.opt_attr(n, 'basis', _p_BasisType)
    v.minimum_value = X.opt_attr(n, 'minimumValue', X.p_double)
    v.maximum_value = X.opt_attr(n, 'maximumValue', X.p_double)
    v.stream_ref = X.req_attr(n, 'streamRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_ReceivedVerifierType = frozenset({'shortDescription', 'name'})


def _r_ReceivedVerifierType(n: X.Node) -> ReceivedVerifierType:
    X.check_attrs(n, _A_ReceivedVerifierType)
    v = ReceivedVerifierType.__new__(ReceivedVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_ReceivedVerifierChoice1(c)
    v.choice2 = _r_ReceivedVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_ReferenceTimeType = frozenset({})


def _r_ReferenceTimeType(n: X.Node) -> ReferenceTimeType:
    X.check_attrs(n, _A_ReferenceTimeType)
    v = ReferenceTimeType.__new__(ReferenceTimeType)
    c = X.Cursor(n)
    v.choice = _r_ReferenceTimeChoice(c)
    c.end()
    return v


_A_RelativeTimeArgumentType = frozenset({"shortDescription", "name", "baseType",
                                         "initialValue"})


def _r_RelativeTimeArgumentType(n: X.Node) -> RelativeTimeArgumentType:
    X.check_attrs(n, _A_RelativeTimeArgumentType)
    v = RelativeTimeArgumentType.__new__(RelativeTimeArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_RelativeTimeDataType = frozenset({"shortDescription", "name", "baseType",
                                     "initialValue"})


def _r_RelativeTimeDataType(n: X.Node) -> RelativeTimeDataType:
    X.check_attrs(n, _A_RelativeTimeDataType)
    v = RelativeTimeDataType.__new__(RelativeTimeDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    c.end()
    return v


_A_RelativeTimeParameterType = frozenset({"shortDescription", "name",
                                          "baseType", "initialValue"})


def _r_RelativeTimeParameterType(n: X.Node) -> RelativeTimeParameterType:
    X.check_attrs(n, _A_RelativeTimeParameterType)
    v = RelativeTimeParameterType.__new__(RelativeTimeParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.encoding = _r_EncodingType(c.take()) if c.peek() == 'Encoding' else None
    v.reference_time = (_r_ReferenceTimeType(c.take())
                        if c.peek() == "ReferenceTime" else None)
    v.default_alarm = (_r_TimeAlarmType(c.take()) if c.peek() == "DefaultAlarm"
                       else None)
    v.context_alarm_list = (_r_TimeContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_RepeatType = frozenset({})


def _r_RepeatType(n: X.Node) -> RepeatType:
    X.check_attrs(n, _A_RepeatType)
    v = RepeatType.__new__(RepeatType)
    c = X.Cursor(n)
    if c.peek() != 'Count':
        raise c.expected('Count')
    v.count = _r_IntegerValueType(c.take())
    v.offset = _r_IntegerValueType(c.take()) if c.peek() == 'Offset' else None
    c.end()
    return v


_A_RestrictionCriteriaType = frozenset({})


def _r_RestrictionCriteriaType(n: X.Node) -> RestrictionCriteriaType:
    X.check_attrs(n, _A_RestrictionCriteriaType)
    v = RestrictionCriteriaType.__new__(RestrictionCriteriaType)
    c = X.Cursor(n)
    v.choice = _r_RestrictionCriteriaChoice(c)
    v.next_container = (_r_ContainerRefType(c.take())
                        if c.peek() == "NextContainer" else None)
    c.end()
    return v


_A_SentFromRangeVerifierType = frozenset({'shortDescription', 'name'})


def _r_SentFromRangeVerifierType(n: X.Node) -> SentFromRangeVerifierType:
    X.check_attrs(n, _A_SentFromRangeVerifierType)
    v = SentFromRangeVerifierType.__new__(SentFromRangeVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_SentFromRangeVerifierChoice1(c)
    v.choice2 = _r_SentFromRangeVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_SequenceContainerType = frozenset({"shortDescription", "name", "abstract",
                                      "idlePattern"})


def _r_SequenceContainerType(n: X.Node) -> SequenceContainerType:
    X.check_attrs(n, _A_SequenceContainerType)
    v = SequenceContainerType.__new__(SequenceContainerType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.abstract = X.opt_attr(n, 'abstract', X.p_bool)
    v.idle_pattern = X.opt_attr(n, 'idlePattern', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.default_rate_in_stream = (_r_RateInStreamType(c.take())
                                if c.peek() == "DefaultRateInStream" else None)
    v.rate_in_stream_set = (_r_RateInStreamSetType(c.take())
                            if c.peek() == "RateInStreamSet" else None)
    v.binary_encoding = (_r_ContainerBinaryDataEncodingType(c.take())
                         if c.peek() == "BinaryEncoding" else None)
    if c.peek() != 'EntryList':
        raise c.expected('EntryList')
    v.entry_list = _r_EntryListType(c.take())
    v.base_container = (_r_BaseContainerType(c.take())
                        if c.peek() == "BaseContainer" else None)
    c.end()
    return v


_A_SequenceEntryType = frozenset({'shortDescription'})


def _r_SequenceEntryType(n: X.Node) -> SequenceEntryType:
    X.check_attrs(n, _A_SequenceEntryType)
    v = SequenceEntryType.__new__(SequenceEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_ServiceRefType = frozenset({'serviceRef'})


def _r_ServiceRefType(n: X.Node) -> ServiceRefType:
    X.check_attrs(n, _A_ServiceRefType)
    v = ServiceRefType.__new__(ServiceRefType)
    v.service_ref = X.req_attr(n, 'serviceRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_ServiceSetType = frozenset({})


def _r_ServiceSetType(n: X.Node) -> ServiceSetType:
    X.check_attrs(n, _A_ServiceSetType)
    v = ServiceSetType.__new__(ServiceSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Service':
        items.append(_r_ServiceType(c.take()))
    if len(items) < 1:
        raise c.too_few('Service', 1, len(items))
    v.service = items
    c.end()
    return v


_A_ServiceType = frozenset({'shortDescription', 'name'})


def _r_ServiceType(n: X.Node) -> ServiceType:
    X.check_attrs(n, _A_ServiceType)
    v = ServiceType.__new__(ServiceType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = _r_ServiceChoice(c)
    c.end()
    return v


_A_SignificanceType = frozenset({"spaceSystemAtRisk", "reasonForWarning",
                                 "consequenceLevel"})


def _r_SignificanceType(n: X.Node) -> SignificanceType:
    X.check_attrs(n, _A_SignificanceType)
    v = SignificanceType.__new__(SignificanceType)
    v.space_system_at_risk = X.opt_attr(n, 'spaceSystemAtRisk', X.p_str)
    v.reason_for_warning = X.opt_attr(n, 'reasonForWarning', X.p_str)
    v.consequence_level = X.opt_attr(n, "consequenceLevel",
                                     _p_ConsequenceLevelType)
    c = X.Cursor(n)
    c.end()
    return v


_A_SimpleAlgorithmType = frozenset({'shortDescription', 'name'})


def _r_SimpleAlgorithmType(n: X.Node) -> SimpleAlgorithmType:
    X.check_attrs(n, _A_SimpleAlgorithmType)
    v = SimpleAlgorithmType.__new__(SimpleAlgorithmType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.algorithm_text = (_r_AlgorithmTextType(c.take())
                        if c.peek() == "AlgorithmText" else None)
    v.external_algorithm_set = (_r_ExternalAlgorithmSetType(c.take())
                                if c.peek() == "ExternalAlgorithmSet" else None)
    c.end()
    return v


_A_SizeInBitsFixedType = frozenset({})


def _r_SizeInBitsFixedType(n: X.Node) -> SizeInBitsFixedType:
    X.check_attrs(n, _A_SizeInBitsFixedType)
    v = SizeInBitsFixedType.__new__(SizeInBitsFixedType)
    c = X.Cursor(n)
    if c.peek() != 'FixedValue':
        raise c.expected('FixedValue')
    v.fixed_value = X.leaf(c.take(), X.p_i64)
    c.end()
    return v


_A_SizeInBitsType = frozenset({})


def _r_SizeInBitsType(n: X.Node) -> SizeInBitsType:
    X.check_attrs(n, _A_SizeInBitsType)
    v = SizeInBitsType.__new__(SizeInBitsType)
    c = X.Cursor(n)
    if c.peek() != 'Fixed':
        raise c.expected('Fixed')
    v.fixed = _r_SizeInBitsFixedType(c.take())
    v.termination_char = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "TerminationChar" else None)
    v.leading_size = (_r_LeadingSizeType(c.take()) if c.peek() == "LeadingSize"
                      else None)
    c.end()
    return v


_A_SpaceSystemType = frozenset({"shortDescription", "name", "systemType",
                                "assetType", "operationalStatus", "xml:base"})


def _r_SpaceSystemType(n: X.Node) -> SpaceSystemType:
    X.check_attrs(n, _A_SpaceSystemType)
    v = SpaceSystemType.__new__(SpaceSystemType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.system_type = X.opt_attr(n, 'systemType', _p_SystemTypeType)
    v.asset_type = X.opt_attr(n, 'assetType', X.p_str)
    v.operational_status = X.opt_attr(n, 'operationalStatus', X.p_str)
    v.xml_base = X.opt_attr(n, 'xml:base', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.header = _r_HeaderType(c.take()) if c.peek() == 'Header' else None
    v.telemetry_meta_data = (_r_TelemetryMetaDataType(c.take())
                             if c.peek() == "TelemetryMetaData" else None)
    v.command_meta_data = (_r_CommandMetaDataType(c.take())
                           if c.peek() == "CommandMetaData" else None)
    v.service_set = (_r_ServiceSetType(c.take()) if c.peek() == "ServiceSet"
                     else None)
    items = []
    while c.peek() == 'SpaceSystem':
        items.append(_r_SpaceSystemType(c.take()))
    v.space_system = items
    c.end()
    return v


_A_SplineCalibratorType = frozenset({"name", "shortDescription", "order",
                                     "extrapolate"})


def _r_SplineCalibratorType(n: X.Node) -> SplineCalibratorType:
    X.check_attrs(n, _A_SplineCalibratorType)
    v = SplineCalibratorType.__new__(SplineCalibratorType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.extrapolate = X.opt_attr(n, 'extrapolate', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() == 'SplinePoint':
        items.append(_r_SplinePointType(c.take()))
    if len(items) < 2:
        raise c.too_few('SplinePoint', 2, len(items))
    v.spline_point = items
    c.end()
    return v


_A_SplinePointType = frozenset({'order', 'raw', 'calibrated'})


def _r_SplinePointType(n: X.Node) -> SplinePointType:
    X.check_attrs(n, _A_SplinePointType)
    v = SplinePointType.__new__(SplinePointType)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.raw = X.req_attr(n, 'raw', X.p_double)
    v.calibrated = X.req_attr(n, 'calibrated', X.p_double)
    c = X.Cursor(n)
    c.end()
    return v


_A_StreamRefType = frozenset({'streamRef'})


def _r_StreamRefType(n: X.Node) -> StreamRefType:
    X.check_attrs(n, _A_StreamRefType)
    v = StreamRefType.__new__(StreamRefType)
    v.stream_ref = X.req_attr(n, 'streamRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_StreamSegmentEntryType = frozenset({"shortDescription", "streamRef", "order",
                                       "sizeInBits"})


def _r_StreamSegmentEntryType(n: X.Node) -> StreamSegmentEntryType:
    X.check_attrs(n, _A_StreamSegmentEntryType)
    v = StreamSegmentEntryType.__new__(StreamSegmentEntryType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.stream_ref = X.req_attr(n, 'streamRef', X.p_str)
    v.order = X.opt_attr(n, 'order', X.p_i64)
    v.size_in_bits = X.req_attr(n, 'sizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.location_in_container_in_bits = _r_LocationInContainerInBitsType(
        c.take()) if c.peek() == 'LocationInContainerInBits' else None
    v.repeat_entry = (_r_RepeatType(c.take()) if c.peek() == "RepeatEntry"
                      else None)
    v.include_condition = (_r_MatchCriteriaType(c.take())
                           if c.peek() == "IncludeCondition" else None)
    v.time_association = (_r_TimeAssociationType(c.take())
                          if c.peek() == "TimeAssociation" else None)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    c.end()
    return v


_A_StreamSetType = frozenset({})


def _r_StreamSetType(n: X.Node) -> StreamSetType:
    X.check_attrs(n, _A_StreamSetType)
    v = StreamSetType.__new__(StreamSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() in StreamSetChoice.NAMES:
        items.append(_r_StreamSetChoice(c))
    if len(items) < 1:
        raise c.too_few("FixedFrameStream|VariableFrameStream|CustomStream", 1,
                        len(items))
    v.items = items
    c.end()
    return v


_A_StringAlarmLevelType = frozenset({'alarmLevel', 'matchPattern'})


def _r_StringAlarmLevelType(n: X.Node) -> StringAlarmLevelType:
    X.check_attrs(n, _A_StringAlarmLevelType)
    v = StringAlarmLevelType.__new__(StringAlarmLevelType)
    v.alarm_level = X.req_attr(n, 'alarmLevel', _p_ConcernLevelsType)
    v.match_pattern = X.req_attr(n, 'matchPattern', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_StringAlarmListType = frozenset({})


def _r_StringAlarmListType(n: X.Node) -> StringAlarmListType:
    X.check_attrs(n, _A_StringAlarmListType)
    v = StringAlarmListType.__new__(StringAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'StringAlarm':
        items.append(_r_StringAlarmLevelType(c.take()))
    if len(items) < 1:
        raise c.too_few('StringAlarm', 1, len(items))
    v.string_alarm = items
    c.end()
    return v


_A_StringAlarmType = frozenset({"name", "shortDescription", "minViolations",
                                "minConformance", "disabled",
                                "defaultAlarmLevel"})


def _r_StringAlarmType(n: X.Node) -> StringAlarmType:
    X.check_attrs(n, _A_StringAlarmType)
    v = StringAlarmType.__new__(StringAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    v.default_alarm_level = X.opt_attr(n, "defaultAlarmLevel",
                                       _p_ConcernLevelsType)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_StringAlarmChoice(c) if c.peek() in StringAlarmChoice.NAMES
                else None)
    v.string_alarm_list = (_r_StringAlarmListType(c.take())
                           if c.peek() == "StringAlarmList" else None)
    c.end()
    return v


_A_StringArgumentType = frozenset({"shortDescription", "name", "baseType",
                                   "initialValue", "restrictionPattern",
                                   "characterWidth"})


def _r_StringArgumentType(n: X.Node) -> StringArgumentType:
    X.check_attrs(n, _A_StringArgumentType)
    v = StringArgumentType.__new__(StringArgumentType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.restriction_pattern = X.opt_attr(n, 'restrictionPattern', X.p_str)
    v.character_width = X.opt_attr(n, 'characterWidth', _p_CharacterWidthType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_StringArgumentChoice(c)
                if c.peek() in StringArgumentChoice.NAMES else None)
    v.size_range_in_characters = (_r_IntegerRangeType(c.take())
                                  if c.peek() == "SizeRangeInCharacters"
                                  else None)
    c.end()
    return v


_A_StringContextAlarmListType = frozenset({})


def _r_StringContextAlarmListType(n: X.Node) -> StringContextAlarmListType:
    X.check_attrs(n, _A_StringContextAlarmListType)
    v = StringContextAlarmListType.__new__(StringContextAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextAlarm':
        items.append(_r_StringContextAlarmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextAlarm', 1, len(items))
    v.context_alarm = items
    c.end()
    return v


_A_StringContextAlarmType = frozenset({"name", "shortDescription",
                                       "minViolations", "minConformance",
                                       "disabled", "defaultAlarmLevel"})


def _r_StringContextAlarmType(n: X.Node) -> StringContextAlarmType:
    X.check_attrs(n, _A_StringContextAlarmType)
    v = StringContextAlarmType.__new__(StringContextAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    v.default_alarm_level = X.opt_attr(n, "defaultAlarmLevel",
                                       _p_ConcernLevelsType)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_StringContextAlarmChoice(c)
                if c.peek() in StringContextAlarmChoice.NAMES else None)
    v.string_alarm_list = (_r_StringAlarmListType(c.take())
                           if c.peek() == "StringAlarmList" else None)
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    c.end()
    return v


_A_StringDataEncodingType = frozenset({'bitOrder', 'byteOrder', 'encoding'})


def _r_StringDataEncodingType(n: X.Node) -> StringDataEncodingType:
    X.check_attrs(n, _A_StringDataEncodingType)
    v = StringDataEncodingType.__new__(StringDataEncodingType)
    v.bit_order = X.opt_attr(n, 'bitOrder', _p_BitOrderType)
    v.byte_order = X.opt_attr(n, 'byteOrder', X.p_str)
    v.encoding = X.opt_attr(n, 'encoding', _p_StringEncodingType)
    c = X.Cursor(n)
    v.error_detect_correct = (_r_ErrorDetectCorrectType(c.take())
                              if c.peek() == "ErrorDetectCorrect" else None)
    v.choice = _r_StringDataEncodingChoice(c)
    c.end()
    return v


_A_StringDataType = frozenset({"shortDescription", "name", "baseType",
                               "initialValue", "restrictionPattern",
                               "characterWidth"})


def _r_StringDataType(n: X.Node) -> StringDataType:
    X.check_attrs(n, _A_StringDataType)
    v = StringDataType.__new__(StringDataType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.restriction_pattern = X.opt_attr(n, 'restrictionPattern', X.p_str)
    v.character_width = X.opt_attr(n, 'characterWidth', _p_CharacterWidthType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_StringDataChoice(c) if c.peek() in StringDataChoice.NAMES
                else None)
    v.size_range_in_characters = (_r_IntegerRangeType(c.take())
                                  if c.peek() == "SizeRangeInCharacters"
                                  else None)
    c.end()
    return v


_A_StringParameterType = frozenset({"shortDescription", "name", "baseType",
                                    "initialValue", "restrictionPattern",
                                    "characterWidth"})


def _r_StringParameterType(n: X.Node) -> StringParameterType:
    X.check_attrs(n, _A_StringParameterType)
    v = StringParameterType.__new__(StringParameterType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.base_type = X.opt_attr(n, 'baseType', X.p_str)
    v.initial_value = X.opt_attr(n, 'initialValue', X.p_str)
    v.restriction_pattern = X.opt_attr(n, 'restrictionPattern', X.p_str)
    v.character_width = X.opt_attr(n, 'characterWidth', _p_CharacterWidthType)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.unit_set = _r_UnitSetType(c.take()) if c.peek() == 'UnitSet' else None
    v.choice = (_r_StringParameterChoice(c)
                if c.peek() in StringParameterChoice.NAMES else None)
    v.size_range_in_characters = (_r_IntegerRangeType(c.take())
                                  if c.peek() == "SizeRangeInCharacters"
                                  else None)
    v.default_alarm = (_r_StringAlarmType(c.take())
                       if c.peek() == "DefaultAlarm" else None)
    v.context_alarm_list = (_r_StringContextAlarmListType(c.take())
                            if c.peek() == "ContextAlarmList" else None)
    c.end()
    return v


_A_SyncPatternType = frozenset({"pattern", "bitLocationFromStartOfContainer",
                                "mask", "maskLengthInBits",
                                "patternLengthInBits"})


def _r_SyncPatternType(n: X.Node) -> SyncPatternType:
    X.check_attrs(n, _A_SyncPatternType)
    v = SyncPatternType.__new__(SyncPatternType)
    v.pattern = X.req_attr(n, 'pattern', X.p_str)
    v.bit_location_from_start_of_container = X.opt_attr(
        n, "bitLocationFromStartOfContainer", X.p_i64)
    v.mask = X.opt_attr(n, 'mask', X.p_str)
    v.mask_length_in_bits = X.opt_attr(n, 'maskLengthInBits', X.p_i64)
    v.pattern_length_in_bits = X.req_attr(n, 'patternLengthInBits', X.p_i64)
    c = X.Cursor(n)
    c.end()
    return v


_A_SyncStrategyType = frozenset({"verifyToLockGoodFrames",
                                 "checkToLockGoodFrames",
                                 "maxBitErrorsInSyncPattern"})


def _r_SyncStrategyType(n: X.Node) -> SyncStrategyType:
    X.check_attrs(n, _A_SyncStrategyType)
    v = SyncStrategyType.__new__(SyncStrategyType)
    v.verify_to_lock_good_frames = X.opt_attr(n, "verifyToLockGoodFrames",
                                              X.p_i64)
    v.check_to_lock_good_frames = X.opt_attr(n, "checkToLockGoodFrames",
                                             X.p_i64)
    v.max_bit_errors_in_sync_pattern = X.opt_attr(
        n, "maxBitErrorsInSyncPattern", X.p_i64)
    c = X.Cursor(n)
    v.auto_invert = (_r_AutoInvertType(c.take()) if c.peek() == "AutoInvert"
                     else None)
    c.end()
    return v


_A_TelemetryMetaDataType = frozenset({})


def _r_TelemetryMetaDataType(n: X.Node) -> TelemetryMetaDataType:
    X.check_attrs(n, _A_TelemetryMetaDataType)
    v = TelemetryMetaDataType.__new__(TelemetryMetaDataType)
    c = X.Cursor(n)
    v.parameter_type_set = (_r_ParameterTypeSetType(c.take())
                            if c.peek() == "ParameterTypeSet" else None)
    v.parameter_set = (_r_ParameterSetType(c.take())
                       if c.peek() == "ParameterSet" else None)
    v.container_set = (_r_ContainerSetType(c.take())
                       if c.peek() == "ContainerSet" else None)
    v.message_set = (_r_MessageSetType(c.take()) if c.peek() == "MessageSet"
                     else None)
    v.stream_set = (_r_StreamSetType(c.take()) if c.peek() == "StreamSet"
                    else None)
    v.algorithm_set = (_r_AlgorithmSetType(c.take())
                       if c.peek() == "AlgorithmSet" else None)
    c.end()
    return v


_A_TermType = frozenset({'coefficient', 'exponent'})


def _r_TermType(n: X.Node) -> TermType:
    X.check_attrs(n, _A_TermType)
    v = TermType.__new__(TermType)
    v.coefficient = X.req_attr(n, 'coefficient', X.p_double)
    v.exponent = X.req_attr(n, 'exponent', X.p_i64)
    c = X.Cursor(n)
    c.end()
    return v


_A_TimeAlarmRangesType = frozenset({"name", "shortDescription", "rangeForm",
                                    "timeUnits"})


def _r_TimeAlarmRangesType(n: X.Node) -> TimeAlarmRangesType:
    X.check_attrs(n, _A_TimeAlarmRangesType)
    v = TimeAlarmRangesType.__new__(TimeAlarmRangesType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.range_form = X.opt_attr(n, 'rangeForm', _p_RangeFormType)
    v.time_units = X.opt_attr(n, 'timeUnits', _p_TimeUnitsType)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.watch_range = (_r_FloatRangeType(c.take()) if c.peek() == "WatchRange"
                     else None)
    v.warning_range = (_r_FloatRangeType(c.take()) if c.peek() == "WarningRange"
                       else None)
    v.distress_range = (_r_FloatRangeType(c.take())
                        if c.peek() == "DistressRange" else None)
    v.critical_range = (_r_FloatRangeType(c.take())
                        if c.peek() == "CriticalRange" else None)
    v.severe_range = (_r_FloatRangeType(c.take()) if c.peek() == "SevereRange"
                      else None)
    c.end()
    return v


_A_TimeAlarmType = frozenset({"name", "shortDescription", "minViolations",
                              "minConformance", "disabled"})


def _r_TimeAlarmType(n: X.Node) -> TimeAlarmType:
    X.check_attrs(n, _A_TimeAlarmType)
    v = TimeAlarmType.__new__(TimeAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_TimeAlarmChoice(c) if c.peek() in TimeAlarmChoice.NAMES
                else None)
    v.static_alarm_ranges = (_r_TimeAlarmRangesType(c.take())
                             if c.peek() == "StaticAlarmRanges" else None)
    v.change_per_second_alarm_ranges = _r_TimeAlarmRangesType(
        c.take()) if c.peek() == 'ChangePerSecondAlarmRanges' else None
    c.end()
    return v


_A_TimeAssociationType = frozenset({"parameterRef", "instance",
                                    "useCalibratedValue", "interpolateTime",
                                    "offset", "unit"})


def _r_TimeAssociationType(n: X.Node) -> TimeAssociationType:
    X.check_attrs(n, _A_TimeAssociationType)
    v = TimeAssociationType.__new__(TimeAssociationType)
    v.parameter_ref = X.req_attr(n, 'parameterRef', X.p_str)
    v.instance = X.opt_attr(n, 'instance', X.p_i64)
    v.use_calibrated_value = X.opt_attr(n, 'useCalibratedValue', X.p_bool)
    v.interpolate_time = X.opt_attr(n, 'interpolateTime', X.p_bool)
    v.offset = X.opt_attr(n, 'offset', X.p_double)
    v.unit = X.opt_attr(n, 'unit', _p_TimeAssociationUnitType)
    c = X.Cursor(n)
    c.end()
    return v


_A_TimeContextAlarmListType = frozenset({})


def _r_TimeContextAlarmListType(n: X.Node) -> TimeContextAlarmListType:
    X.check_attrs(n, _A_TimeContextAlarmListType)
    v = TimeContextAlarmListType.__new__(TimeContextAlarmListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ContextAlarm':
        items.append(_r_TimeContextAlarmType(c.take()))
    if len(items) < 1:
        raise c.too_few('ContextAlarm', 1, len(items))
    v.context_alarm = items
    c.end()
    return v


_A_TimeContextAlarmType = frozenset(
    {"name", "shortDescription", "minViolations", "minConformance", "disabled"})


def _r_TimeContextAlarmType(n: X.Node) -> TimeContextAlarmType:
    X.check_attrs(n, _A_TimeContextAlarmType)
    v = TimeContextAlarmType.__new__(TimeContextAlarmType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.min_violations = X.opt_attr(n, 'minViolations', X.p_i64)
    v.min_conformance = X.opt_attr(n, 'minConformance', X.p_i64)
    v.disabled = X.opt_attr(n, 'disabled', X.p_bool)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = (_r_TimeContextAlarmChoice(c)
                if c.peek() in TimeContextAlarmChoice.NAMES else None)
    v.static_alarm_ranges = (_r_TimeAlarmRangesType(c.take())
                             if c.peek() == "StaticAlarmRanges" else None)
    v.change_per_second_alarm_ranges = _r_TimeAlarmRangesType(
        c.take()) if c.peek() == 'ChangePerSecondAlarmRanges' else None
    if c.peek() != 'ContextMatch':
        raise c.expected('ContextMatch')
    v.context_match = _r_ContextMatchType(c.take())
    c.end()
    return v


_A_ToStringType = frozenset({})


def _r_ToStringType(n: X.Node) -> ToStringType:
    X.check_attrs(n, _A_ToStringType)
    v = ToStringType.__new__(ToStringType)
    c = X.Cursor(n)
    if c.peek() != 'NumberFormat':
        raise c.expected('NumberFormat')
    v.number_format = _r_NumberFormatType(c.take())
    c.end()
    return v


_A_TransferredToRangeVerifierType = frozenset({'shortDescription', 'name'})


def _r_TransferredToRangeVerifierType(
        n: X.Node) -> TransferredToRangeVerifierType:
    X.check_attrs(n, _A_TransferredToRangeVerifierType)
    v = TransferredToRangeVerifierType.__new__(TransferredToRangeVerifierType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.opt_attr(n, 'name', X.p_str)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice1 = _r_TransferredToRangeVerifierChoice1(c)
    v.choice2 = _r_TransferredToRangeVerifierChoice2(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_TransmissionConstraintListType = frozenset({})


def _r_TransmissionConstraintListType(
        n: X.Node) -> TransmissionConstraintListType:
    X.check_attrs(n, _A_TransmissionConstraintListType)
    v = TransmissionConstraintListType.__new__(TransmissionConstraintListType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'TransmissionConstraint':
        items.append(_r_TransmissionConstraintType(c.take()))
    if len(items) < 1:
        raise c.too_few('TransmissionConstraint', 1, len(items))
    v.transmission_constraint = items
    c.end()
    return v


_A_TransmissionConstraintType = frozenset({'timeOut', 'suspendable'})


def _r_TransmissionConstraintType(n: X.Node) -> TransmissionConstraintType:
    X.check_attrs(n, _A_TransmissionConstraintType)
    v = TransmissionConstraintType.__new__(TransmissionConstraintType)
    v.time_out = X.opt_attr(n, 'timeOut', X.p_str)
    v.suspendable = X.opt_attr(n, 'suspendable', X.p_bool)
    c = X.Cursor(n)
    v.choice = _r_TransmissionConstraintChoice(c)
    v.argument_restriction_list = (_r_ArgumentAssignmentListType(c.take())
                                   if c.peek() == "ArgumentRestrictionList"
                                   else None)
    c.end()
    return v


_A_TriggerSetType = frozenset({'name', 'triggerRate'})


def _r_TriggerSetType(n: X.Node) -> TriggerSetType:
    X.check_attrs(n, _A_TriggerSetType)
    v = TriggerSetType.__new__(TriggerSetType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.trigger_rate = X.opt_attr(n, 'triggerRate', X.p_i64)
    c = X.Cursor(n)
    items = []
    while c.peek() in TriggerSetChoice.NAMES:
        items.append(_r_TriggerSetChoice(c))
    if len(items) < 1:
        raise c.too_few("OnParameterUpdateTrigger|OnContainerUpdateTrigger|"
                        "OnPeriodicRateTrigger", 1, len(items))
    v.items = items
    c.end()
    return v


_A_TriggeredMathOperationType = frozenset({"name", "shortDescription",
                                           "outputParameterRef"})


def _r_TriggeredMathOperationType(n: X.Node) -> TriggeredMathOperationType:
    X.check_attrs(n, _A_TriggeredMathOperationType)
    v = TriggeredMathOperationType.__new__(TriggeredMathOperationType)
    v.name = X.opt_attr(n, 'name', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.output_parameter_ref = X.req_attr(n, 'outputParameterRef', X.p_str)
    c = X.Cursor(n)
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    items = []
    while c.peek() in TriggeredMathOperationChoice.NAMES:
        items.append(_r_TriggeredMathOperationChoice(c))
    if len(items) < 1:
        raise c.too_few("ValueOperand|ThisParameterOperand|Operator|"
                        "ParameterInstanceRefOperand", 1, len(items))
    v.items = items
    if c.peek() != 'TriggerSet':
        raise c.expected('TriggerSet')
    v.trigger_set = _r_TriggerSetType(c.take())
    c.end()
    return v


_A_UnitSetType = frozenset({})


def _r_UnitSetType(n: X.Node) -> UnitSetType:
    X.check_attrs(n, _A_UnitSetType)
    v = UnitSetType.__new__(UnitSetType)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'Unit':
        items.append(_r_UnitType(c.take()))
    v.unit = items
    c.end()
    return v


_A_UnitType = frozenset({'power', 'factor', 'description', 'form'})


def _r_UnitType(n: X.Node) -> UnitType:
    X.check_attrs(n, _A_UnitType)
    v = UnitType.__new__(UnitType)
    v.power = X.opt_attr(n, 'power', X.p_double)
    v.factor = X.opt_attr(n, 'factor', X.p_str)
    v.description = X.opt_attr(n, 'description', X.p_str)
    v.form = X.opt_attr(n, 'form', _p_UnitFormType)
    v.value = X.value(n, X.text_of(n), X.p_str)
    return v


_A_ValidFloatRangeSetType = frozenset({'validRangeAppliesToCalibrated'})


def _r_ValidFloatRangeSetType(n: X.Node) -> ValidFloatRangeSetType:
    X.check_attrs(n, _A_ValidFloatRangeSetType)
    v = ValidFloatRangeSetType.__new__(ValidFloatRangeSetType)
    v.valid_range_applies_to_calibrated = X.opt_attr(
        n, "validRangeAppliesToCalibrated", X.p_bool)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ValidRange':
        items.append(_r_FloatRangeType(c.take()))
    if len(items) < 1:
        raise c.too_few('ValidRange', 1, len(items))
    v.valid_range = items
    c.end()
    return v


_A_ValidIntegerRangeSetType = frozenset({'validRangeAppliesToCalibrated'})


def _r_ValidIntegerRangeSetType(n: X.Node) -> ValidIntegerRangeSetType:
    X.check_attrs(n, _A_ValidIntegerRangeSetType)
    v = ValidIntegerRangeSetType.__new__(ValidIntegerRangeSetType)
    v.valid_range_applies_to_calibrated = X.opt_attr(
        n, "validRangeAppliesToCalibrated", X.p_bool)
    c = X.Cursor(n)
    items = []
    while c.peek() == 'ValidRange':
        items.append(_r_IntegerRangeType(c.take()))
    if len(items) < 1:
        raise c.too_few('ValidRange', 1, len(items))
    v.valid_range = items
    c.end()
    return v


_A_ValueEnumerationType = frozenset({"value", "maxValue", "label",
                                     "shortDescription"})


def _r_ValueEnumerationType(n: X.Node) -> ValueEnumerationType:
    X.check_attrs(n, _A_ValueEnumerationType)
    v = ValueEnumerationType.__new__(ValueEnumerationType)
    v.value = X.req_attr(n, 'value', X.p_i64)
    v.max_value = X.opt_attr(n, 'maxValue', X.p_i64)
    v.label = X.req_attr(n, 'label', X.p_str)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_A_VariableFrameStreamType = frozenset({"shortDescription", "name",
                                        "bitRateInBPS", "pcmType", "inverted"})


def _r_VariableFrameStreamType(n: X.Node) -> VariableFrameStreamType:
    X.check_attrs(n, _A_VariableFrameStreamType)
    v = VariableFrameStreamType.__new__(VariableFrameStreamType)
    v.short_description = X.opt_attr(n, 'shortDescription', X.p_str)
    v.name = X.req_attr(n, 'name', X.p_str)
    v.bit_rate_in_bps = X.opt_attr(n, 'bitRateInBPS', X.p_double)
    v.pcm_type = X.opt_attr(n, 'pcmType', _p_PCMType)
    v.inverted = X.opt_attr(n, 'inverted', X.p_bool)
    c = X.Cursor(n)
    v.long_description = (X.leaf(c.take(), X.p_str)
                          if c.peek() == "LongDescription" else None)
    v.alias_set = _r_AliasSetType(c.take()) if c.peek() == 'AliasSet' else None
    v.ancillary_data_set = (_r_AncillaryDataSetType(c.take())
                            if c.peek() == "AncillaryDataSet" else None)
    v.choice = _r_VariableFrameStreamChoice(c)
    v.stream_ref = (_r_StreamRefType(c.take()) if c.peek() == "StreamRef"
                    else None)
    if c.peek() != 'SyncStrategy':
        raise c.expected('SyncStrategy')
    v.sync_strategy = _r_VariableFrameSyncStrategyType(c.take())
    c.end()
    return v


_A_VariableFrameSyncStrategyType = frozenset({"verifyToLockGoodFrames",
                                              "checkToLockGoodFrames",
                                              "maxBitErrorsInSyncPattern"})


def _r_VariableFrameSyncStrategyType(
        n: X.Node) -> VariableFrameSyncStrategyType:
    X.check_attrs(n, _A_VariableFrameSyncStrategyType)
    v = VariableFrameSyncStrategyType.__new__(VariableFrameSyncStrategyType)
    v.verify_to_lock_good_frames = X.opt_attr(n, "verifyToLockGoodFrames",
                                              X.p_i64)
    v.check_to_lock_good_frames = X.opt_attr(n, "checkToLockGoodFrames",
                                             X.p_i64)
    v.max_bit_errors_in_sync_pattern = X.opt_attr(
        n, "maxBitErrorsInSyncPattern", X.p_i64)
    c = X.Cursor(n)
    v.auto_invert = (_r_AutoInvertType(c.take()) if c.peek() == "AutoInvert"
                     else None)
    if c.peek() != 'Flag':
        raise c.expected('Flag')
    v.flag = _r_FlagType(c.take())
    c.end()
    return v


_A_VariableStringType = frozenset({'maxSizeInBits'})


def _r_VariableStringType(n: X.Node) -> VariableStringType:
    X.check_attrs(n, _A_VariableStringType)
    v = VariableStringType.__new__(VariableStringType)
    v.max_size_in_bits = X.req_attr(n, 'maxSizeInBits', X.p_i64)
    c = X.Cursor(n)
    v.choice1 = (_r_VariableStringChoice1(c)
                 if c.peek() in VariableStringChoice1.NAMES else None)
    v.choice2 = _r_VariableStringChoice2(c)
    c.end()
    return v


_A_VerifierSetType = frozenset({})


def _r_VerifierSetType(n: X.Node) -> VerifierSetType:
    X.check_attrs(n, _A_VerifierSetType)
    v = VerifierSetType.__new__(VerifierSetType)
    c = X.Cursor(n)
    v.transferred_to_range_verifier = _r_TransferredToRangeVerifierType(
        c.take()) if c.peek() == 'TransferredToRangeVerifier' else None
    v.sent_from_range_verifier = (_r_SentFromRangeVerifierType(c.take())
                                  if c.peek() == "SentFromRangeVerifier"
                                  else None)
    v.received_verifier = (_r_ReceivedVerifierType(c.take())
                           if c.peek() == "ReceivedVerifier" else None)
    v.accepted_verifier = (_r_AcceptedVerifierType(c.take())
                           if c.peek() == "AcceptedVerifier" else None)
    v.queued_verifier = (_r_QueuedVerifierType(c.take())
                         if c.peek() == "QueuedVerifier" else None)
    items = []
    while c.peek() == 'ExecutionVerifier':
        items.append(_r_ExecutionVerifierType(c.take()))
    v.execution_verifier = items
    items = []
    while c.peek() == 'CompleteVerifier':
        items.append(_r_CompleteVerifierType(c.take()))
    v.complete_verifier = items
    v.failed_verifier = (_r_FailedVerifierType(c.take())
                         if c.peek() == "FailedVerifier" else None)
    c.end()
    return v


_A_XORType = frozenset({'bitsFromReference', 'reference', 'parameterRef'})


def _r_XORType(n: X.Node) -> XORType:
    X.check_attrs(n, _A_XORType)
    v = XORType.__new__(XORType)
    v.bits_from_reference = X.opt_attr(n, 'bitsFromReference', X.p_i64)
    v.reference = X.opt_attr(n, 'reference', _p_ReferencePointType)
    v.parameter_ref = X.opt_attr(n, 'parameterRef', X.p_str)
    c = X.Cursor(n)
    c.end()
    return v


_RC_ANDedConditionsChoice = {"Condition": _r_ComparisonCheckType,
                             "ORedConditions": _r_ORedConditionsType}


def _r_ANDedConditionsChoice(c: X.Cursor) -> ANDedConditionsChoice:
    k = c.peek()
    fn = _RC_ANDedConditionsChoice.get(k)
    if fn is None:
        raise c.expected('Condition|ORedConditions')
    return ANDedConditionsChoice(k, fn(c.take()))


_RC_AcceptedVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_AcceptedVerifierChoice1(c: X.Cursor) -> AcceptedVerifierChoice1:
    k = c.peek()
    fn = _RC_AcceptedVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return AcceptedVerifierChoice1(k, fn(c.take()))


_RC_AcceptedVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_AcceptedVerifierChoice2(c: X.Cursor) -> AcceptedVerifierChoice2:
    k = c.peek()
    fn = _RC_AcceptedVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return AcceptedVerifierChoice2(k, fn(c.take()))


_RC_AlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                   "CustomAlarm": _r_CustomAlarmType}


def _r_AlarmChoice(c: X.Cursor) -> AlarmChoice:
    k = c.peek()
    fn = _RC_AlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return AlarmChoice(k, fn(c.take()))


_RC_AlgorithmSetChoice = {"CustomAlgorithm": _r_InputOutputTriggerAlgorithmType,
                          "MathAlgorithm": _r_MathAlgorithmType}


def _r_AlgorithmSetChoice(c: X.Cursor) -> AlgorithmSetChoice:
    k = c.peek()
    fn = _RC_AlgorithmSetChoice.get(k)
    if fn is None:
        raise c.expected('CustomAlgorithm|MathAlgorithm')
    return AlgorithmSetChoice(k, fn(c.take()))


_RC_ArgumentANDedConditionsChoice = {
    "Condition": _r_ArgumentComparisonCheckType,
    "ORedConditions": _r_ArgumentORedConditionsType}


def _r_ArgumentANDedConditionsChoice(
        c: X.Cursor) -> ArgumentANDedConditionsChoice:
    k = c.peek()
    fn = _RC_ArgumentANDedConditionsChoice.get(k)
    if fn is None:
        raise c.expected('Condition|ORedConditions')
    return ArgumentANDedConditionsChoice(k, fn(c.take()))


_RC_ArgumentBaseDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentBaseDataChoice(c: X.Cursor) -> ArgumentBaseDataChoice:
    k = c.peek()
    fn = _RC_ArgumentBaseDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentBaseDataChoice(k, fn(c.take()))


_RC_ArgumentBinaryDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentBinaryDataChoice(c: X.Cursor) -> ArgumentBinaryDataChoice:
    k = c.peek()
    fn = _RC_ArgumentBinaryDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentBinaryDataChoice(k, fn(c.take()))


_RC_ArgumentBooleanDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentBooleanDataChoice(c: X.Cursor) -> ArgumentBooleanDataChoice:
    k = c.peek()
    fn = _RC_ArgumentBooleanDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentBooleanDataChoice(k, fn(c.take()))


_RC_ArgumentBooleanExpressionChoice = {
    "Condition": _r_ArgumentComparisonCheckType,
    "ANDedConditions": _r_ArgumentANDedConditionsType,
    "ORedConditions": _r_ArgumentORedConditionsType}


def _r_ArgumentBooleanExpressionChoice(
        c: X.Cursor) -> ArgumentBooleanExpressionChoice:
    k = c.peek()
    fn = _RC_ArgumentBooleanExpressionChoice.get(k)
    if fn is None:
        raise c.expected('Condition|ANDedConditions|ORedConditions')
    return ArgumentBooleanExpressionChoice(k, fn(c.take()))


_RC_ArgumentComparisonCheckChoice1 = {
    "ParameterInstanceRef": _r_ParameterInstanceRefType,
    "ArgumentInstanceRef": _r_ArgumentInstanceRefType}


def _r_ArgumentComparisonCheckChoice1(
        c: X.Cursor) -> ArgumentComparisonCheckChoice1:
    k = c.peek()
    fn = _RC_ArgumentComparisonCheckChoice1.get(k)
    if fn is None:
        raise c.expected('ParameterInstanceRef|ArgumentInstanceRef')
    return ArgumentComparisonCheckChoice1(k, fn(c.take()))


_RC_ArgumentComparisonCheckChoice2 = {
    "ParameterInstanceRef": _r_ParameterInstanceRefType,
    "ArgumentInstanceRef": _r_ArgumentInstanceRefType,
    "Value": lambda n: X.leaf(n, X.p_str)}


def _r_ArgumentComparisonCheckChoice2(
        c: X.Cursor) -> ArgumentComparisonCheckChoice2:
    k = c.peek()
    fn = _RC_ArgumentComparisonCheckChoice2.get(k)
    if fn is None:
        raise c.expected('ParameterInstanceRef|ArgumentInstanceRef|Value')
    return ArgumentComparisonCheckChoice2(k, fn(c.take()))


_RC_ArgumentComparisonChoice = {
    "ParameterInstanceRef": _r_ParameterInstanceRefType,
    "ArgumentInstanceRef": _r_ArgumentInstanceRefType}


def _r_ArgumentComparisonChoice(c: X.Cursor) -> ArgumentComparisonChoice:
    k = c.peek()
    fn = _RC_ArgumentComparisonChoice.get(k)
    if fn is None:
        raise c.expected('ParameterInstanceRef|ArgumentInstanceRef')
    return ArgumentComparisonChoice(k, fn(c.take()))


_RC_ArgumentDiscreteLookupChoice = {
    "Comparison": _r_ArgumentComparisonType,
    "ComparisonList": _r_ArgumentComparisonListType,
    "BooleanExpression": _r_ArgumentBooleanExpressionType,
    "CustomAlgorithm": _r_ArgumentInputAlgorithmType}


def _r_ArgumentDiscreteLookupChoice(
        c: X.Cursor) -> ArgumentDiscreteLookupChoice:
    k = c.peek()
    fn = _RC_ArgumentDiscreteLookupChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return ArgumentDiscreteLookupChoice(k, fn(c.take()))


_RC_ArgumentDynamicValueChoice = {
    "ArgumentInstanceRef": _r_ArgumentInstanceRefType,
    "ParameterInstanceRef": _r_ParameterInstanceRefType}


def _r_ArgumentDynamicValueChoice(c: X.Cursor) -> ArgumentDynamicValueChoice:
    k = c.peek()
    fn = _RC_ArgumentDynamicValueChoice.get(k)
    if fn is None:
        raise c.expected('ArgumentInstanceRef|ParameterInstanceRef')
    return ArgumentDynamicValueChoice(k, fn(c.take()))


_RC_ArgumentEnumeratedDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentEnumeratedDataChoice(
        c: X.Cursor) -> ArgumentEnumeratedDataChoice:
    k = c.peek()
    fn = _RC_ArgumentEnumeratedDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentEnumeratedDataChoice(k, fn(c.take()))


_RC_ArgumentFloatDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentFloatDataChoice(c: X.Cursor) -> ArgumentFloatDataChoice:
    k = c.peek()
    fn = _RC_ArgumentFloatDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentFloatDataChoice(k, fn(c.take()))


_RC_ArgumentInputSetChoice = {
    "InputParameterInstanceRef": _r_InputParameterInstanceRefType,
    "InputArgumentInstanceRef": _r_ArgumentInstanceRefType,
    "Constant": _r_ConstantType}


def _r_ArgumentInputSetChoice(c: X.Cursor) -> ArgumentInputSetChoice:
    k = c.peek()
    fn = _RC_ArgumentInputSetChoice.get(k)
    if fn is None:
        raise c.expected("InputParameterInstanceRef|InputArgumentInstanceRef|"
                         "Constant")
    return ArgumentInputSetChoice(k, fn(c.take()))


_RC_ArgumentIntegerDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentIntegerDataChoice(c: X.Cursor) -> ArgumentIntegerDataChoice:
    k = c.peek()
    fn = _RC_ArgumentIntegerDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentIntegerDataChoice(k, fn(c.take()))


_RC_ArgumentIntegerValueChoice = {
    "FixedValue": lambda n: X.leaf(n, X.p_i64),
    "DynamicValue": _r_ArgumentDynamicValueType,
    "DiscreteLookupList": _r_ArgumentDiscreteLookupListType}


def _r_ArgumentIntegerValueChoice(c: X.Cursor) -> ArgumentIntegerValueChoice:
    k = c.peek()
    fn = _RC_ArgumentIntegerValueChoice.get(k)
    if fn is None:
        raise c.expected('FixedValue|DynamicValue|DiscreteLookupList')
    return ArgumentIntegerValueChoice(k, fn(c.take()))


_RC_ArgumentLocationInContainerInBitsChoice = {
    "FixedValue": lambda n: X.leaf(n, X.p_i64),
    "DynamicValue": _r_ArgumentDynamicValueType,
    "DiscreteLookupList": _r_ArgumentDiscreteLookupListType}


def _r_ArgumentLocationInContainerInBitsChoice(
        c: X.Cursor) -> ArgumentLocationInContainerInBitsChoice:
    k = c.peek()
    fn = _RC_ArgumentLocationInContainerInBitsChoice.get(k)
    if fn is None:
        raise c.expected('FixedValue|DynamicValue|DiscreteLookupList')
    return ArgumentLocationInContainerInBitsChoice(k, fn(c.take()))


_RC_ArgumentMatchCriteriaChoice = {
    "Comparison": _r_ArgumentComparisonType,
    "ComparisonList": _r_ArgumentComparisonListType,
    "BooleanExpression": _r_ArgumentBooleanExpressionType,
    "CustomAlgorithm": _r_ArgumentInputAlgorithmType}


def _r_ArgumentMatchCriteriaChoice(c: X.Cursor) -> ArgumentMatchCriteriaChoice:
    k = c.peek()
    fn = _RC_ArgumentMatchCriteriaChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return ArgumentMatchCriteriaChoice(k, fn(c.take()))


_RC_ArgumentMathOperationChoice = {
    "ValueOperand": lambda n: X.leaf(n, X.p_str),
    "ThisParameterOperand": lambda n: X.leaf(n, X.p_str),
    "Operator": lambda n: X.leaf(n, _p_MathOperatorsType),
    "ParameterInstanceRefOperand": _r_ParameterInstanceRefType,
    "ArgumentInstanceRefOperand": _r_ArgumentInstanceRefType}


def _r_ArgumentMathOperationChoice(c: X.Cursor) -> ArgumentMathOperationChoice:
    k = c.peek()
    fn = _RC_ArgumentMathOperationChoice.get(k)
    if fn is None:
        raise c.expected("ValueOperand|ThisParameterOperand|Operator|"
                         "ParameterInstanceRefOperand|"
                         "ArgumentInstanceRefOperand")
    return ArgumentMathOperationChoice(k, fn(c.take()))


_RC_ArgumentORedConditionsChoice = {
    "Condition": _r_ArgumentComparisonCheckType,
    "ANDedConditions": _r_ArgumentANDedConditionsType}


def _r_ArgumentORedConditionsChoice(
        c: X.Cursor) -> ArgumentORedConditionsChoice:
    k = c.peek()
    fn = _RC_ArgumentORedConditionsChoice.get(k)
    if fn is None:
        raise c.expected('Condition|ANDedConditions')
    return ArgumentORedConditionsChoice(k, fn(c.take()))


_RC_ArgumentStringDataChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_ArgumentStringDataChoice(c: X.Cursor) -> ArgumentStringDataChoice:
    k = c.peek()
    fn = _RC_ArgumentStringDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return ArgumentStringDataChoice(k, fn(c.take()))


_RC_ArgumentStringDataEncodingChoice = {
    "SizeInBits": _r_SizeInBitsType, "Variable": _r_ArgumentVariableStringType}


def _r_ArgumentStringDataEncodingChoice(
        c: X.Cursor) -> ArgumentStringDataEncodingChoice:
    k = c.peek()
    fn = _RC_ArgumentStringDataEncodingChoice.get(k)
    if fn is None:
        raise c.expected('SizeInBits|Variable')
    return ArgumentStringDataEncodingChoice(k, fn(c.take()))


_RC_ArgumentTypeSetChoice = {
    "StringArgumentType": _r_StringArgumentType,
    "EnumeratedArgumentType": _r_EnumeratedArgumentType,
    "IntegerArgumentType": _r_IntegerArgumentType,
    "BinaryArgumentType": _r_BinaryArgumentType,
    "FloatArgumentType": _r_FloatArgumentType,
    "BooleanArgumentType": _r_BooleanArgumentType,
    "RelativeTimeArgumentType": _r_RelativeTimeArgumentType,
    "AbsoluteTimeArgumentType": _r_AbsoluteTimeArgumentType,
    "ArrayArgumentType": _r_ArrayArgumentType,
    "AggregateArgumentType": _r_AggregateArgumentType}


def _r_ArgumentTypeSetChoice(c: X.Cursor) -> ArgumentTypeSetChoice:
    k = c.peek()
    fn = _RC_ArgumentTypeSetChoice.get(k)
    if fn is None:
        raise c.expected("StringArgumentType|EnumeratedArgumentType|"
                         "IntegerArgumentType|BinaryArgumentType|"
                         "FloatArgumentType|BooleanArgumentType|"
                         "RelativeTimeArgumentType|AbsoluteTimeArgumentType|"
                         "ArrayArgumentType|AggregateArgumentType")
    return ArgumentTypeSetChoice(k, fn(c.take()))


_RC_ArgumentVariableStringChoice1 = {
    "DynamicValue": _r_ArgumentDynamicValueType,
    "DiscreteLookupList": _r_ArgumentDiscreteLookupListType}


def _r_ArgumentVariableStringChoice1(
        c: X.Cursor) -> ArgumentVariableStringChoice1:
    k = c.peek()
    fn = _RC_ArgumentVariableStringChoice1.get(k)
    if fn is None:
        raise c.expected('DynamicValue|DiscreteLookupList')
    return ArgumentVariableStringChoice1(k, fn(c.take()))


_RC_ArgumentVariableStringChoice2 = {
    "LeadingSize": _r_LeadingSizeType,
    "TerminationChar": lambda n: X.leaf(n, X.p_str)}


def _r_ArgumentVariableStringChoice2(
        c: X.Cursor) -> ArgumentVariableStringChoice2:
    k = c.peek()
    fn = _RC_ArgumentVariableStringChoice2.get(k)
    if fn is None:
        raise c.expected('LeadingSize|TerminationChar')
    return ArgumentVariableStringChoice2(k, fn(c.take()))


_RC_BaseDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                      "FloatDataEncoding": _r_FloatDataEncodingType,
                      "IntegerDataEncoding": _r_IntegerDataEncodingType,
                      "StringDataEncoding": _r_StringDataEncodingType}


def _r_BaseDataChoice(c: X.Cursor) -> BaseDataChoice:
    k = c.peek()
    fn = _RC_BaseDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BaseDataChoice(k, fn(c.take()))


_RC_BinaryAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                         "CustomAlarm": _r_CustomAlarmType}


def _r_BinaryAlarmChoice(c: X.Cursor) -> BinaryAlarmChoice:
    k = c.peek()
    fn = _RC_BinaryAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return BinaryAlarmChoice(k, fn(c.take()))


_RC_BinaryArgumentChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_BinaryArgumentChoice(c: X.Cursor) -> BinaryArgumentChoice:
    k = c.peek()
    fn = _RC_BinaryArgumentChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BinaryArgumentChoice(k, fn(c.take()))


_RC_BinaryContextAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                                "CustomAlarm": _r_CustomAlarmType}


def _r_BinaryContextAlarmChoice(c: X.Cursor) -> BinaryContextAlarmChoice:
    k = c.peek()
    fn = _RC_BinaryContextAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return BinaryContextAlarmChoice(k, fn(c.take()))


_RC_BinaryDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                        "FloatDataEncoding": _r_FloatDataEncodingType,
                        "IntegerDataEncoding": _r_IntegerDataEncodingType,
                        "StringDataEncoding": _r_StringDataEncodingType}


def _r_BinaryDataChoice(c: X.Cursor) -> BinaryDataChoice:
    k = c.peek()
    fn = _RC_BinaryDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BinaryDataChoice(k, fn(c.take()))


_RC_BinaryParameterChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                             "FloatDataEncoding": _r_FloatDataEncodingType,
                             "IntegerDataEncoding": _r_IntegerDataEncodingType,
                             "StringDataEncoding": _r_StringDataEncodingType}


def _r_BinaryParameterChoice(c: X.Cursor) -> BinaryParameterChoice:
    k = c.peek()
    fn = _RC_BinaryParameterChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BinaryParameterChoice(k, fn(c.take()))


_RC_BooleanAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                          "CustomAlarm": _r_CustomAlarmType}


def _r_BooleanAlarmChoice(c: X.Cursor) -> BooleanAlarmChoice:
    k = c.peek()
    fn = _RC_BooleanAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return BooleanAlarmChoice(k, fn(c.take()))


_RC_BooleanArgumentChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_BooleanArgumentChoice(c: X.Cursor) -> BooleanArgumentChoice:
    k = c.peek()
    fn = _RC_BooleanArgumentChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BooleanArgumentChoice(k, fn(c.take()))


_RC_BooleanContextAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                                 "CustomAlarm": _r_CustomAlarmType}


def _r_BooleanContextAlarmChoice(c: X.Cursor) -> BooleanContextAlarmChoice:
    k = c.peek()
    fn = _RC_BooleanContextAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return BooleanContextAlarmChoice(k, fn(c.take()))


_RC_BooleanDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                         "FloatDataEncoding": _r_FloatDataEncodingType,
                         "IntegerDataEncoding": _r_IntegerDataEncodingType,
                         "StringDataEncoding": _r_StringDataEncodingType}


def _r_BooleanDataChoice(c: X.Cursor) -> BooleanDataChoice:
    k = c.peek()
    fn = _RC_BooleanDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BooleanDataChoice(k, fn(c.take()))


_RC_BooleanExpressionChoice = {"Condition": _r_ComparisonCheckType,
                               "ANDedConditions": _r_ANDedConditionsType,
                               "ORedConditions": _r_ORedConditionsType}


def _r_BooleanExpressionChoice(c: X.Cursor) -> BooleanExpressionChoice:
    k = c.peek()
    fn = _RC_BooleanExpressionChoice.get(k)
    if fn is None:
        raise c.expected('Condition|ANDedConditions|ORedConditions')
    return BooleanExpressionChoice(k, fn(c.take()))


_RC_BooleanParameterChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                              "FloatDataEncoding": _r_FloatDataEncodingType,
                              "IntegerDataEncoding": _r_IntegerDataEncodingType,
                              "StringDataEncoding": _r_StringDataEncodingType}


def _r_BooleanParameterChoice(c: X.Cursor) -> BooleanParameterChoice:
    k = c.peek()
    fn = _RC_BooleanParameterChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return BooleanParameterChoice(k, fn(c.take()))


_RC_CalibratorChoice = {
    "SplineCalibrator": _r_SplineCalibratorType,
    "PolynomialCalibrator": _r_PolynomialCalibratorType,
    "MathOperationCalibrator": _r_MathOperationCalibratorType}


def _r_CalibratorChoice(c: X.Cursor) -> CalibratorChoice:
    k = c.peek()
    fn = _RC_CalibratorChoice.get(k)
    if fn is None:
        raise c.expected("SplineCalibrator|PolynomialCalibrator|"
                         "MathOperationCalibrator")
    return CalibratorChoice(k, fn(c.take()))


_RC_CommandContainerEntryListChoice = {
    "ParameterRefEntry": _r_ArgumentParameterRefEntryType,
    "ParameterSegmentRefEntry": _r_ArgumentParameterSegmentRefEntryType,
    "ContainerRefEntry": _r_ArgumentContainerRefEntryType,
    "ContainerSegmentRefEntry": _r_ArgumentContainerSegmentRefEntryType,
    "StreamSegmentEntry": _r_ArgumentStreamSegmentEntryType,
    "IndirectParameterRefEntry": _r_ArgumentIndirectParameterRefEntryType,
    "ArrayParameterRefEntry": _r_ArgumentArrayParameterRefEntryType,
    "ArgumentRefEntry": _r_ArgumentArgumentRefEntryType,
    "ArrayArgumentRefEntry": _r_ArgumentArrayArgumentRefEntryType,
    "FixedValueEntry": _r_ArgumentFixedValueEntryType}


def _r_CommandContainerEntryListChoice(
        c: X.Cursor) -> CommandContainerEntryListChoice:
    k = c.peek()
    fn = _RC_CommandContainerEntryListChoice.get(k)
    if fn is None:
        raise c.expected("ParameterRefEntry|ParameterSegmentRefEntry|"
                         "ContainerRefEntry|ContainerSegmentRefEntry|"
                         "StreamSegmentEntry|IndirectParameterRefEntry|"
                         "ArrayParameterRefEntry|ArgumentRefEntry|"
                         "ArrayArgumentRefEntry|FixedValueEntry")
    return CommandContainerEntryListChoice(k, fn(c.take()))


_RC_CommandVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_CommandVerifierChoice1(c: X.Cursor) -> CommandVerifierChoice1:
    k = c.peek()
    fn = _RC_CommandVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return CommandVerifierChoice1(k, fn(c.take()))


_RC_CommandVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_CommandVerifierChoice2(c: X.Cursor) -> CommandVerifierChoice2:
    k = c.peek()
    fn = _RC_CommandVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return CommandVerifierChoice2(k, fn(c.take()))


_RC_ComparisonCheckChoice = {
    "ParameterInstanceRef": _r_ParameterInstanceRefType,
    "Value": lambda n: X.leaf(n, X.p_str)}


def _r_ComparisonCheckChoice(c: X.Cursor) -> ComparisonCheckChoice:
    k = c.peek()
    fn = _RC_ComparisonCheckChoice.get(k)
    if fn is None:
        raise c.expected('ParameterInstanceRef|Value')
    return ComparisonCheckChoice(k, fn(c.take()))


_RC_CompleteVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_CompleteVerifierChoice1(c: X.Cursor) -> CompleteVerifierChoice1:
    k = c.peek()
    fn = _RC_CompleteVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return CompleteVerifierChoice1(k, fn(c.take()))


_RC_CompleteVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_CompleteVerifierChoice2(c: X.Cursor) -> CompleteVerifierChoice2:
    k = c.peek()
    fn = _RC_CompleteVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return CompleteVerifierChoice2(k, fn(c.take()))


_RC_ContextMatchChoice = {"Comparison": _r_ComparisonType,
                          "ComparisonList": _r_ComparisonListType,
                          "BooleanExpression": _r_BooleanExpressionType,
                          "CustomAlgorithm": _r_InputAlgorithmType}


def _r_ContextMatchChoice(c: X.Cursor) -> ContextMatchChoice:
    k = c.peek()
    fn = _RC_ContextMatchChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return ContextMatchChoice(k, fn(c.take()))


_RC_DiscreteLookupChoice = {"Comparison": _r_ComparisonType,
                            "ComparisonList": _r_ComparisonListType,
                            "BooleanExpression": _r_BooleanExpressionType,
                            "CustomAlgorithm": _r_InputAlgorithmType}


def _r_DiscreteLookupChoice(c: X.Cursor) -> DiscreteLookupChoice:
    k = c.peek()
    fn = _RC_DiscreteLookupChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return DiscreteLookupChoice(k, fn(c.take()))


_RC_EncodingChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                      "FloatDataEncoding": _r_FloatDataEncodingType,
                      "IntegerDataEncoding": _r_IntegerDataEncodingType,
                      "StringDataEncoding": _r_StringDataEncodingType}


def _r_EncodingChoice(c: X.Cursor) -> EncodingChoice:
    k = c.peek()
    fn = _RC_EncodingChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return EncodingChoice(k, fn(c.take()))


_RC_EntryListChoice = {
    "ParameterRefEntry": _r_ParameterRefEntryType,
    "ParameterSegmentRefEntry": _r_ParameterSegmentRefEntryType,
    "ContainerRefEntry": _r_ContainerRefEntryType,
    "ContainerSegmentRefEntry": _r_ContainerSegmentRefEntryType,
    "StreamSegmentEntry": _r_StreamSegmentEntryType,
    "IndirectParameterRefEntry": _r_IndirectParameterRefEntryType,
    "ArrayParameterRefEntry": _r_ArrayParameterRefEntryType}


def _r_EntryListChoice(c: X.Cursor) -> EntryListChoice:
    k = c.peek()
    fn = _RC_EntryListChoice.get(k)
    if fn is None:
        raise c.expected("ParameterRefEntry|ParameterSegmentRefEntry|"
                         "ContainerRefEntry|ContainerSegmentRefEntry|"
                         "StreamSegmentEntry|IndirectParameterRefEntry|"
                         "ArrayParameterRefEntry")
    return EntryListChoice(k, fn(c.take()))


_RC_EnumeratedArgumentChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_EnumeratedArgumentChoice(c: X.Cursor) -> EnumeratedArgumentChoice:
    k = c.peek()
    fn = _RC_EnumeratedArgumentChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return EnumeratedArgumentChoice(k, fn(c.take()))


_RC_EnumeratedDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                            "FloatDataEncoding": _r_FloatDataEncodingType,
                            "IntegerDataEncoding": _r_IntegerDataEncodingType,
                            "StringDataEncoding": _r_StringDataEncodingType}


def _r_EnumeratedDataChoice(c: X.Cursor) -> EnumeratedDataChoice:
    k = c.peek()
    fn = _RC_EnumeratedDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return EnumeratedDataChoice(k, fn(c.take()))


_RC_EnumeratedParameterChoice = {
    "BinaryDataEncoding": _r_BinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_StringDataEncodingType}


def _r_EnumeratedParameterChoice(c: X.Cursor) -> EnumeratedParameterChoice:
    k = c.peek()
    fn = _RC_EnumeratedParameterChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return EnumeratedParameterChoice(k, fn(c.take()))


_RC_EnumerationAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                              "CustomAlarm": _r_CustomAlarmType}


def _r_EnumerationAlarmChoice(c: X.Cursor) -> EnumerationAlarmChoice:
    k = c.peek()
    fn = _RC_EnumerationAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return EnumerationAlarmChoice(k, fn(c.take()))


_RC_EnumerationContextAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                                     "CustomAlarm": _r_CustomAlarmType}


def _r_EnumerationContextAlarmChoice(
        c: X.Cursor) -> EnumerationContextAlarmChoice:
    k = c.peek()
    fn = _RC_EnumerationContextAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return EnumerationContextAlarmChoice(k, fn(c.take()))


_RC_ErrorDetectCorrectChoice = {"Checksum": _r_ChecksumType, "CRC": _r_CRCType,
                                "XOR": _r_XORType, "Parity": _r_ParityType}


def _r_ErrorDetectCorrectChoice(c: X.Cursor) -> ErrorDetectCorrectChoice:
    k = c.peek()
    fn = _RC_ErrorDetectCorrectChoice.get(k)
    if fn is None:
        raise c.expected('Checksum|CRC|XOR|Parity')
    return ErrorDetectCorrectChoice(k, fn(c.take()))


_RC_ExecutionVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_ExecutionVerifierChoice1(c: X.Cursor) -> ExecutionVerifierChoice1:
    k = c.peek()
    fn = _RC_ExecutionVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return ExecutionVerifierChoice1(k, fn(c.take()))


_RC_ExecutionVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_ExecutionVerifierChoice2(c: X.Cursor) -> ExecutionVerifierChoice2:
    k = c.peek()
    fn = _RC_ExecutionVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return ExecutionVerifierChoice2(k, fn(c.take()))


_RC_FailedVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_FailedVerifierChoice1(c: X.Cursor) -> FailedVerifierChoice1:
    k = c.peek()
    fn = _RC_FailedVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return FailedVerifierChoice1(k, fn(c.take()))


_RC_FailedVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_FailedVerifierChoice2(c: X.Cursor) -> FailedVerifierChoice2:
    k = c.peek()
    fn = _RC_FailedVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return FailedVerifierChoice2(k, fn(c.take()))


_RC_FixedFrameStreamChoice = {"ContainerRef": _r_ContainerRefType,
                              "ServiceRef": _r_ServiceRefType}


def _r_FixedFrameStreamChoice(c: X.Cursor) -> FixedFrameStreamChoice:
    k = c.peek()
    fn = _RC_FixedFrameStreamChoice.get(k)
    if fn is None:
        raise c.expected('ContainerRef|ServiceRef')
    return FixedFrameStreamChoice(k, fn(c.take()))


_RC_FloatArgumentChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_FloatArgumentChoice(c: X.Cursor) -> FloatArgumentChoice:
    k = c.peek()
    fn = _RC_FloatArgumentChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return FloatArgumentChoice(k, fn(c.take()))


_RC_FloatDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                       "FloatDataEncoding": _r_FloatDataEncodingType,
                       "IntegerDataEncoding": _r_IntegerDataEncodingType,
                       "StringDataEncoding": _r_StringDataEncodingType}


def _r_FloatDataChoice(c: X.Cursor) -> FloatDataChoice:
    k = c.peek()
    fn = _RC_FloatDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return FloatDataChoice(k, fn(c.take()))


_RC_FloatParameterChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                            "FloatDataEncoding": _r_FloatDataEncodingType,
                            "IntegerDataEncoding": _r_IntegerDataEncodingType,
                            "StringDataEncoding": _r_StringDataEncodingType}


def _r_FloatParameterChoice(c: X.Cursor) -> FloatParameterChoice:
    k = c.peek()
    fn = _RC_FloatParameterChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return FloatParameterChoice(k, fn(c.take()))


_RC_FrameStreamChoice = {"ContainerRef": _r_ContainerRefType,
                         "ServiceRef": _r_ServiceRefType}


def _r_FrameStreamChoice(c: X.Cursor) -> FrameStreamChoice:
    k = c.peek()
    fn = _RC_FrameStreamChoice.get(k)
    if fn is None:
        raise c.expected('ContainerRef|ServiceRef')
    return FrameStreamChoice(k, fn(c.take()))


_RC_InputSetChoice = {
    "InputParameterInstanceRef": _r_InputParameterInstanceRefType,
    "Constant": _r_ConstantType}


def _r_InputSetChoice(c: X.Cursor) -> InputSetChoice:
    k = c.peek()
    fn = _RC_InputSetChoice.get(k)
    if fn is None:
        raise c.expected('InputParameterInstanceRef|Constant')
    return InputSetChoice(k, fn(c.take()))


_RC_IntegerArgumentChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_IntegerArgumentChoice(c: X.Cursor) -> IntegerArgumentChoice:
    k = c.peek()
    fn = _RC_IntegerArgumentChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return IntegerArgumentChoice(k, fn(c.take()))


_RC_IntegerDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                         "FloatDataEncoding": _r_FloatDataEncodingType,
                         "IntegerDataEncoding": _r_IntegerDataEncodingType,
                         "StringDataEncoding": _r_StringDataEncodingType}


def _r_IntegerDataChoice(c: X.Cursor) -> IntegerDataChoice:
    k = c.peek()
    fn = _RC_IntegerDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return IntegerDataChoice(k, fn(c.take()))


_RC_IntegerParameterChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                              "FloatDataEncoding": _r_FloatDataEncodingType,
                              "IntegerDataEncoding": _r_IntegerDataEncodingType,
                              "StringDataEncoding": _r_StringDataEncodingType}


def _r_IntegerParameterChoice(c: X.Cursor) -> IntegerParameterChoice:
    k = c.peek()
    fn = _RC_IntegerParameterChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return IntegerParameterChoice(k, fn(c.take()))


_RC_IntegerValueChoice = {"FixedValue": lambda n: X.leaf(n, X.p_i64),
                          "DynamicValue": _r_DynamicValueType,
                          "DiscreteLookupList": _r_DiscreteLookupListType}


def _r_IntegerValueChoice(c: X.Cursor) -> IntegerValueChoice:
    k = c.peek()
    fn = _RC_IntegerValueChoice.get(k)
    if fn is None:
        raise c.expected('FixedValue|DynamicValue|DiscreteLookupList')
    return IntegerValueChoice(k, fn(c.take()))


_RC_LocationInContainerInBitsChoice = {
    "FixedValue": lambda n: X.leaf(n, X.p_i64),
    "DynamicValue": _r_DynamicValueType,
    "DiscreteLookupList": _r_DiscreteLookupListType}


def _r_LocationInContainerInBitsChoice(
        c: X.Cursor) -> LocationInContainerInBitsChoice:
    k = c.peek()
    fn = _RC_LocationInContainerInBitsChoice.get(k)
    if fn is None:
        raise c.expected('FixedValue|DynamicValue|DiscreteLookupList')
    return LocationInContainerInBitsChoice(k, fn(c.take()))


_RC_MatchCriteriaChoice = {"Comparison": _r_ComparisonType,
                           "ComparisonList": _r_ComparisonListType,
                           "BooleanExpression": _r_BooleanExpressionType,
                           "CustomAlgorithm": _r_InputAlgorithmType}


def _r_MatchCriteriaChoice(c: X.Cursor) -> MatchCriteriaChoice:
    k = c.peek()
    fn = _RC_MatchCriteriaChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return MatchCriteriaChoice(k, fn(c.take()))


_RC_MathOperationCalibratorChoice = {
    "ValueOperand": lambda n: X.leaf(n, X.p_str),
    "ThisParameterOperand": lambda n: X.leaf(n, X.p_str),
    "Operator": lambda n: X.leaf(n, _p_MathOperatorsType),
    "ParameterInstanceRefOperand": _r_ParameterInstanceRefType}


def _r_MathOperationCalibratorChoice(
        c: X.Cursor) -> MathOperationCalibratorChoice:
    k = c.peek()
    fn = _RC_MathOperationCalibratorChoice.get(k)
    if fn is None:
        raise c.expected("ValueOperand|ThisParameterOperand|Operator|"
                         "ParameterInstanceRefOperand")
    return MathOperationCalibratorChoice(k, fn(c.take()))


_RC_MathOperationChoice = {
    "ValueOperand": lambda n: X.leaf(n, X.p_str),
    "ThisParameterOperand": lambda n: X.leaf(n, X.p_str),
    "Operator": lambda n: X.leaf(n, _p_MathOperatorsType),
    "ParameterInstanceRefOperand": _r_ParameterInstanceRefType}


def _r_MathOperationChoice(c: X.Cursor) -> MathOperationChoice:
    k = c.peek()
    fn = _RC_MathOperationChoice.get(k)
    if fn is None:
        raise c.expected("ValueOperand|ThisParameterOperand|Operator|"
                         "ParameterInstanceRefOperand")
    return MathOperationChoice(k, fn(c.take()))


_RC_MetaCommandSetChoice = {"MetaCommand": _r_MetaCommandType,
                            "MetaCommandRef": lambda n: X.leaf(n, X.p_str),
                            "BlockMetaCommand": _r_BlockMetaCommandType}


def _r_MetaCommandSetChoice(c: X.Cursor) -> MetaCommandSetChoice:
    k = c.peek()
    fn = _RC_MetaCommandSetChoice.get(k)
    if fn is None:
        raise c.expected('MetaCommand|MetaCommandRef|BlockMetaCommand')
    return MetaCommandSetChoice(k, fn(c.take()))


_RC_NumericAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                          "CustomAlarm": _r_CustomAlarmType}


def _r_NumericAlarmChoice(c: X.Cursor) -> NumericAlarmChoice:
    k = c.peek()
    fn = _RC_NumericAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return NumericAlarmChoice(k, fn(c.take()))


_RC_NumericContextAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                                 "CustomAlarm": _r_CustomAlarmType}


def _r_NumericContextAlarmChoice(c: X.Cursor) -> NumericContextAlarmChoice:
    k = c.peek()
    fn = _RC_NumericContextAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return NumericContextAlarmChoice(k, fn(c.take()))


_RC_ORedConditionsChoice = {"Condition": _r_ComparisonCheckType,
                            "ANDedConditions": _r_ANDedConditionsType}


def _r_ORedConditionsChoice(c: X.Cursor) -> ORedConditionsChoice:
    k = c.peek()
    fn = _RC_ORedConditionsChoice.get(k)
    if fn is None:
        raise c.expected('Condition|ANDedConditions')
    return ORedConditionsChoice(k, fn(c.take()))


_RC_ParameterSetChoice = {"Parameter": _r_ParameterType,
                          "ParameterRef": _r_ParameterRefType}


def _r_ParameterSetChoice(c: X.Cursor) -> ParameterSetChoice:
    k = c.peek()
    fn = _RC_ParameterSetChoice.get(k)
    if fn is None:
        raise c.expected('Parameter|ParameterRef')
    return ParameterSetChoice(k, fn(c.take()))


_RC_ParameterToSetChoice = {"Derivation": _r_ArgumentMathOperationType,
                            "NewValue": lambda n: X.leaf(n, X.p_str)}


def _r_ParameterToSetChoice(c: X.Cursor) -> ParameterToSetChoice:
    k = c.peek()
    fn = _RC_ParameterToSetChoice.get(k)
    if fn is None:
        raise c.expected('Derivation|NewValue')
    return ParameterToSetChoice(k, fn(c.take()))


_RC_ParameterTypeSetChoice = {
    "StringParameterType": _r_StringParameterType,
    "EnumeratedParameterType": _r_EnumeratedParameterType,
    "IntegerParameterType": _r_IntegerParameterType,
    "BinaryParameterType": _r_BinaryParameterType,
    "FloatParameterType": _r_FloatParameterType,
    "BooleanParameterType": _r_BooleanParameterType,
    "RelativeTimeParameterType": _r_RelativeTimeParameterType,
    "AbsoluteTimeParameterType": _r_AbsoluteTimeParameterType,
    "ArrayParameterType": _r_ArrayParameterType,
    "AggregateParameterType": _r_AggregateParameterType}


def _r_ParameterTypeSetChoice(c: X.Cursor) -> ParameterTypeSetChoice:
    k = c.peek()
    fn = _RC_ParameterTypeSetChoice.get(k)
    if fn is None:
        raise c.expected("StringParameterType|EnumeratedParameterType|"
                         "IntegerParameterType|BinaryParameterType|"
                         "FloatParameterType|BooleanParameterType|"
                         "RelativeTimeParameterType|AbsoluteTimeParameterType|"
                         "ArrayParameterType|AggregateParameterType")
    return ParameterTypeSetChoice(k, fn(c.take()))


_RC_PercentCompleteChoice = {"FixedValue": lambda n: X.leaf(n, X.p_double),
                             "DynamicValue": _r_DynamicValueType}


def _r_PercentCompleteChoice(c: X.Cursor) -> PercentCompleteChoice:
    k = c.peek()
    fn = _RC_PercentCompleteChoice.get(k)
    if fn is None:
        raise c.expected('FixedValue|DynamicValue')
    return PercentCompleteChoice(k, fn(c.take()))


_RC_QueuedVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_QueuedVerifierChoice1(c: X.Cursor) -> QueuedVerifierChoice1:
    k = c.peek()
    fn = _RC_QueuedVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return QueuedVerifierChoice1(k, fn(c.take()))


_RC_QueuedVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_QueuedVerifierChoice2(c: X.Cursor) -> QueuedVerifierChoice2:
    k = c.peek()
    fn = _RC_QueuedVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return QueuedVerifierChoice2(k, fn(c.take()))


_RC_ReceivedVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_ReceivedVerifierChoice1(c: X.Cursor) -> ReceivedVerifierChoice1:
    k = c.peek()
    fn = _RC_ReceivedVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return ReceivedVerifierChoice1(k, fn(c.take()))


_RC_ReceivedVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_ReceivedVerifierChoice2(c: X.Cursor) -> ReceivedVerifierChoice2:
    k = c.peek()
    fn = _RC_ReceivedVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return ReceivedVerifierChoice2(k, fn(c.take()))


_RC_ReferenceTimeChoice = {"OffsetFrom": _r_ParameterInstanceRefType,
                           "Epoch": lambda n: X.leaf(n, X.p_str)}


def _r_ReferenceTimeChoice(c: X.Cursor) -> ReferenceTimeChoice:
    k = c.peek()
    fn = _RC_ReferenceTimeChoice.get(k)
    if fn is None:
        raise c.expected('OffsetFrom|Epoch')
    return ReferenceTimeChoice(k, fn(c.take()))


_RC_RestrictionCriteriaChoice = {"Comparison": _r_ComparisonType,
                                 "ComparisonList": _r_ComparisonListType,
                                 "BooleanExpression": _r_BooleanExpressionType,
                                 "CustomAlgorithm": _r_InputAlgorithmType}


def _r_RestrictionCriteriaChoice(c: X.Cursor) -> RestrictionCriteriaChoice:
    k = c.peek()
    fn = _RC_RestrictionCriteriaChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return RestrictionCriteriaChoice(k, fn(c.take()))


_RC_SentFromRangeVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_SentFromRangeVerifierChoice1(
        c: X.Cursor) -> SentFromRangeVerifierChoice1:
    k = c.peek()
    fn = _RC_SentFromRangeVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return SentFromRangeVerifierChoice1(k, fn(c.take()))


_RC_SentFromRangeVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_SentFromRangeVerifierChoice2(
        c: X.Cursor) -> SentFromRangeVerifierChoice2:
    k = c.peek()
    fn = _RC_SentFromRangeVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return SentFromRangeVerifierChoice2(k, fn(c.take()))


_RC_ServiceChoice = {"MessageRefSet": _r_MessageRefSetType,
                     "ContainerRefSet": _r_ContainerRefSetType}


def _r_ServiceChoice(c: X.Cursor) -> ServiceChoice:
    k = c.peek()
    fn = _RC_ServiceChoice.get(k)
    if fn is None:
        raise c.expected('MessageRefSet|ContainerRefSet')
    return ServiceChoice(k, fn(c.take()))


_RC_StreamSetChoice = {"FixedFrameStream": _r_FixedFrameStreamType,
                       "VariableFrameStream": _r_VariableFrameStreamType,
                       "CustomStream": _r_CustomStreamType}


def _r_StreamSetChoice(c: X.Cursor) -> StreamSetChoice:
    k = c.peek()
    fn = _RC_StreamSetChoice.get(k)
    if fn is None:
        raise c.expected('FixedFrameStream|VariableFrameStream|CustomStream')
    return StreamSetChoice(k, fn(c.take()))


_RC_StringAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                         "CustomAlarm": _r_CustomAlarmType}


def _r_StringAlarmChoice(c: X.Cursor) -> StringAlarmChoice:
    k = c.peek()
    fn = _RC_StringAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return StringAlarmChoice(k, fn(c.take()))


_RC_StringArgumentChoice = {
    "BinaryDataEncoding": _r_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _r_FloatDataEncodingType,
    "IntegerDataEncoding": _r_IntegerDataEncodingType,
    "StringDataEncoding": _r_ArgumentStringDataEncodingType}


def _r_StringArgumentChoice(c: X.Cursor) -> StringArgumentChoice:
    k = c.peek()
    fn = _RC_StringArgumentChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return StringArgumentChoice(k, fn(c.take()))


_RC_StringContextAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                                "CustomAlarm": _r_CustomAlarmType}


def _r_StringContextAlarmChoice(c: X.Cursor) -> StringContextAlarmChoice:
    k = c.peek()
    fn = _RC_StringContextAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return StringContextAlarmChoice(k, fn(c.take()))


_RC_StringDataChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                        "FloatDataEncoding": _r_FloatDataEncodingType,
                        "IntegerDataEncoding": _r_IntegerDataEncodingType,
                        "StringDataEncoding": _r_StringDataEncodingType}


def _r_StringDataChoice(c: X.Cursor) -> StringDataChoice:
    k = c.peek()
    fn = _RC_StringDataChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return StringDataChoice(k, fn(c.take()))


_RC_StringDataEncodingChoice = {"SizeInBits": _r_SizeInBitsType,
                                "Variable": _r_VariableStringType}


def _r_StringDataEncodingChoice(c: X.Cursor) -> StringDataEncodingChoice:
    k = c.peek()
    fn = _RC_StringDataEncodingChoice.get(k)
    if fn is None:
        raise c.expected('SizeInBits|Variable')
    return StringDataEncodingChoice(k, fn(c.take()))


_RC_StringParameterChoice = {"BinaryDataEncoding": _r_BinaryDataEncodingType,
                             "FloatDataEncoding": _r_FloatDataEncodingType,
                             "IntegerDataEncoding": _r_IntegerDataEncodingType,
                             "StringDataEncoding": _r_StringDataEncodingType}


def _r_StringParameterChoice(c: X.Cursor) -> StringParameterChoice:
    k = c.peek()
    fn = _RC_StringParameterChoice.get(k)
    if fn is None:
        raise c.expected("BinaryDataEncoding|FloatDataEncoding|"
                         "IntegerDataEncoding|StringDataEncoding")
    return StringParameterChoice(k, fn(c.take()))


_RC_TimeAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                       "CustomAlarm": _r_CustomAlarmType}


def _r_TimeAlarmChoice(c: X.Cursor) -> TimeAlarmChoice:
    k = c.peek()
    fn = _RC_TimeAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return TimeAlarmChoice(k, fn(c.take()))


_RC_TimeContextAlarmChoice = {"AlarmConditions": _r_AlarmConditionsType,
                              "CustomAlarm": _r_CustomAlarmType}


def _r_TimeContextAlarmChoice(c: X.Cursor) -> TimeContextAlarmChoice:
    k = c.peek()
    fn = _RC_TimeContextAlarmChoice.get(k)
    if fn is None:
        raise c.expected('AlarmConditions|CustomAlarm')
    return TimeContextAlarmChoice(k, fn(c.take()))


_RC_TransferredToRangeVerifierChoice1 = {
    "ComparisonList": _r_ComparisonListType,
    "ContainerRef": _r_ContainerRefType,
    "ParameterValueChange": _r_ParameterValueChangeType,
    "CustomAlgorithm": _r_InputAlgorithmType,
    "BooleanExpression": _r_BooleanExpressionType,
    "Comparison": _r_ComparisonType}


def _r_TransferredToRangeVerifierChoice1(
        c: X.Cursor) -> TransferredToRangeVerifierChoice1:
    k = c.peek()
    fn = _RC_TransferredToRangeVerifierChoice1.get(k)
    if fn is None:
        raise c.expected("ComparisonList|ContainerRef|ParameterValueChange|"
                         "CustomAlgorithm|BooleanExpression|Comparison")
    return TransferredToRangeVerifierChoice1(k, fn(c.take()))


_RC_TransferredToRangeVerifierChoice2 = {
    "CheckWindow": _r_CheckWindowType,
    "CheckWindowAlgorithms": _r_CheckWindowAlgorithmsType}


def _r_TransferredToRangeVerifierChoice2(
        c: X.Cursor) -> TransferredToRangeVerifierChoice2:
    k = c.peek()
    fn = _RC_TransferredToRangeVerifierChoice2.get(k)
    if fn is None:
        raise c.expected('CheckWindow|CheckWindowAlgorithms')
    return TransferredToRangeVerifierChoice2(k, fn(c.take()))


_RC_TransmissionConstraintChoice = {
    "Comparison": _r_ComparisonType, "ComparisonList": _r_ComparisonListType,
    "BooleanExpression": _r_BooleanExpressionType,
    "CustomAlgorithm": _r_InputAlgorithmType}


def _r_TransmissionConstraintChoice(
        c: X.Cursor) -> TransmissionConstraintChoice:
    k = c.peek()
    fn = _RC_TransmissionConstraintChoice.get(k)
    if fn is None:
        raise c.expected("Comparison|ComparisonList|BooleanExpression|"
                         "CustomAlgorithm")
    return TransmissionConstraintChoice(k, fn(c.take()))


_RC_TriggerSetChoice = {
    "OnParameterUpdateTrigger": _r_OnParameterUpdateTriggerType,
    "OnContainerUpdateTrigger": _r_OnContainerUpdateTriggerType,
    "OnPeriodicRateTrigger": _r_OnPeriodicRateTriggerType}


def _r_TriggerSetChoice(c: X.Cursor) -> TriggerSetChoice:
    k = c.peek()
    fn = _RC_TriggerSetChoice.get(k)
    if fn is None:
        raise c.expected("OnParameterUpdateTrigger|OnContainerUpdateTrigger|"
                         "OnPeriodicRateTrigger")
    return TriggerSetChoice(k, fn(c.take()))


_RC_TriggeredMathOperationChoice = {
    "ValueOperand": lambda n: X.leaf(n, X.p_str),
    "ThisParameterOperand": lambda n: X.leaf(n, X.p_str),
    "Operator": lambda n: X.leaf(n, _p_MathOperatorsType),
    "ParameterInstanceRefOperand": _r_ParameterInstanceRefType}


def _r_TriggeredMathOperationChoice(
        c: X.Cursor) -> TriggeredMathOperationChoice:
    k = c.peek()
    fn = _RC_TriggeredMathOperationChoice.get(k)
    if fn is None:
        raise c.expected("ValueOperand|ThisParameterOperand|Operator|"
                         "ParameterInstanceRefOperand")
    return TriggeredMathOperationChoice(k, fn(c.take()))


_RC_VariableFrameStreamChoice = {"ContainerRef": _r_ContainerRefType,
                                 "ServiceRef": _r_ServiceRefType}


def _r_VariableFrameStreamChoice(c: X.Cursor) -> VariableFrameStreamChoice:
    k = c.peek()
    fn = _RC_VariableFrameStreamChoice.get(k)
    if fn is None:
        raise c.expected('ContainerRef|ServiceRef')
    return VariableFrameStreamChoice(k, fn(c.take()))


_RC_VariableStringChoice1 = {"DynamicValue": _r_DynamicValueType,
                             "DiscreteLookupList": _r_DiscreteLookupListType}


def _r_VariableStringChoice1(c: X.Cursor) -> VariableStringChoice1:
    k = c.peek()
    fn = _RC_VariableStringChoice1.get(k)
    if fn is None:
        raise c.expected('DynamicValue|DiscreteLookupList')
    return VariableStringChoice1(k, fn(c.take()))


_RC_VariableStringChoice2 = {"LeadingSize": _r_LeadingSizeType,
                             "TerminationChar": lambda n: X.leaf(n, X.p_str)}


def _r_VariableStringChoice2(c: X.Cursor) -> VariableStringChoice2:
    k = c.peek()
    fn = _RC_VariableStringChoice2.get(k)
    if fn is None:
        raise c.expected('LeadingSize|TerminationChar')
    return VariableStringChoice2(k, fn(c.take()))

# -- serialization ------------------------------------------------------------


def _w_ANDedConditionsType(w: X.Writer, v: ANDedConditionsType,
                           tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 2, -1,
                  "ANDedConditionsType.ANDedConditionsChoice")
    for e in v.items:
        _w_ANDedConditionsChoice(w, e)
    w.end(tag)


def _w_AbsoluteTimeArgumentType(w: X.Writer, v: AbsoluteTimeArgumentType,
                                tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_AbsoluteTimeDataType(w: X.Writer, v: AbsoluteTimeDataType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_AbsoluteTimeParameterType(w: X.Writer, v: AbsoluteTimeParameterType,
                                 tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_AcceptedVerifierType(w: X.Writer, v: AcceptedVerifierType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_AcceptedVerifierChoice1(w, v.choice1)
    _w_AcceptedVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_AggregateArgumentType(w: X.Writer, v: AggregateArgumentType,
                             tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_MemberListType(w, v.member_list, 'MemberList')
    w.end(tag)


def _w_AggregateDataType(w: X.Writer, v: AggregateDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_MemberListType(w, v.member_list, 'MemberList')
    w.end(tag)


def _w_AggregateParameterType(w: X.Writer, v: AggregateParameterType,
                              tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_MemberListType(w, v.member_list, 'MemberList')
    w.end(tag)


def _w_AlarmConditionsType(w: X.Writer, v: AlarmConditionsType,
                           tag: str) -> None:
    w.start(tag)
    if v.watch_alarm is not None:
        _w_MatchCriteriaType(w, v.watch_alarm, 'WatchAlarm')
    if v.warning_alarm is not None:
        _w_MatchCriteriaType(w, v.warning_alarm, 'WarningAlarm')
    if v.distress_alarm is not None:
        _w_MatchCriteriaType(w, v.distress_alarm, 'DistressAlarm')
    if v.critical_alarm is not None:
        _w_MatchCriteriaType(w, v.critical_alarm, 'CriticalAlarm')
    if v.severe_alarm is not None:
        _w_MatchCriteriaType(w, v.severe_alarm, 'SevereAlarm')
    w.end(tag)


def _w_AlarmMultiRangesType(w: X.Writer, v: AlarmMultiRangesType,
                            tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.range), 1, -1, 'AlarmMultiRangesType.Range')
    for e in v.range:
        _w_MultiRangeType(w, e, 'Range')
    w.end(tag)


def _w_AlarmRangesType(w: X.Writer, v: AlarmRangesType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.range_form is not None:
        w.attr('rangeForm', v.range_form.value)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.watch_range is not None:
        _w_FloatRangeType(w, v.watch_range, 'WatchRange')
    if v.warning_range is not None:
        _w_FloatRangeType(w, v.warning_range, 'WarningRange')
    if v.distress_range is not None:
        _w_FloatRangeType(w, v.distress_range, 'DistressRange')
    if v.critical_range is not None:
        _w_FloatRangeType(w, v.critical_range, 'CriticalRange')
    if v.severe_range is not None:
        _w_FloatRangeType(w, v.severe_range, 'SevereRange')
    w.end(tag)


def _w_AlarmType(w: X.Writer, v: AlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_AlarmChoice(w, v.choice)
    w.end(tag)


def _w_AlgorithmSetType(w: X.Writer, v: AlgorithmSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1, 'AlgorithmSetType.AlgorithmSetChoice')
    for e in v.items:
        _w_AlgorithmSetChoice(w, e)
    w.end(tag)


def _w_AlgorithmTextType(w: X.Writer, v: AlgorithmTextType, tag: str) -> None:
    w.start(tag)
    if v.language is not None:
        w.attr('language', v.language)
    w.text(v.value)
    w.end(tag)


def _w_AliasSetType(w: X.Writer, v: AliasSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.alias), 1, -1, 'AliasSetType.Alias')
    for e in v.alias:
        _w_AliasType(w, e, 'Alias')
    w.end(tag)


def _w_AliasType(w: X.Writer, v: AliasType, tag: str) -> None:
    w.start(tag)
    w.attr('nameSpace', v.name_space)
    w.attr('alias', v.alias)
    w.end(tag)


def _w_AncillaryDataSetType(w: X.Writer, v: AncillaryDataSetType,
                            tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.ancillary_data), 1, -1,
                  "AncillaryDataSetType.AncillaryData")
    for e in v.ancillary_data:
        _w_AncillaryDataType(w, e, 'AncillaryData')
    w.end(tag)


def _w_AncillaryDataType(w: X.Writer, v: AncillaryDataType, tag: str) -> None:
    w.start(tag)
    w.attr('name', v.name)
    if v.mime_type is not None:
        w.attr('mimeType', v.mime_type)
    if v.href is not None:
        w.attr('href', v.href)
    w.text(v.value)
    w.end(tag)


def _w_ArgumentANDedConditionsType(w: X.Writer, v: ArgumentANDedConditionsType,
                                   tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 2, -1,
                  "ArgumentANDedConditionsType.ArgumentANDedConditionsChoice")
    for e in v.items:
        _w_ArgumentANDedConditionsChoice(w, e)
    w.end(tag)


def _w_ArgumentAbsoluteTimeDataType(
        w: X.Writer, v: ArgumentAbsoluteTimeDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_ArgumentArgumentRefEntryType(
        w: X.Writer, v: ArgumentArgumentRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('argumentRef', v.argument_ref)
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentArrayArgumentRefEntryType(
        w: X.Writer, v: ArgumentArrayArgumentRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('argumentRef', v.argument_ref)
    if v.last_entry_for_this_array_instance is not None:
        w.attr("lastEntryForThisArrayInstance",
               X.f_bool(v.last_entry_for_this_array_instance))
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.dimension_list is not None:
        _w_ArgumentDimensionListType(w, v.dimension_list, 'DimensionList')
    w.end(tag)


def _w_ArgumentArrayParameterRefEntryType(
        w: X.Writer, v: ArgumentArrayParameterRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('parameterRef', v.parameter_ref)
    if v.last_entry_for_this_array_instance is not None:
        w.attr("lastEntryForThisArrayInstance",
               X.f_bool(v.last_entry_for_this_array_instance))
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.dimension_list is not None:
        _w_DimensionListType(w, v.dimension_list, 'DimensionList')
    w.end(tag)


def _w_ArgumentAssignmentListType(w: X.Writer, v: ArgumentAssignmentListType,
                                  tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.argument_assignment), 1, -1,
                  "ArgumentAssignmentListType.ArgumentAssignment")
    for e in v.argument_assignment:
        _w_ArgumentAssignmentType(w, e, 'ArgumentAssignment')
    w.end(tag)


def _w_ArgumentAssignmentType(w: X.Writer, v: ArgumentAssignmentType,
                              tag: str) -> None:
    w.start(tag)
    w.attr('argumentName', v.argument_name)
    w.attr('argumentValue', v.argument_value)
    w.end(tag)


def _w_ArgumentBaseDataType(w: X.Writer, v: ArgumentBaseDataType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentBaseDataChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentBaseTimeDataType(w: X.Writer, v: ArgumentBaseTimeDataType,
                                tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_ArgumentBinaryDataEncodingType(
        w: X.Writer, v: ArgumentBinaryDataEncodingType, tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    _w_ArgumentIntegerValueType(w, v.size_in_bits, 'SizeInBits')
    if v.from_binary_transform_algorithm is not None:
        _w_ArgumentInputAlgorithmType(w, v.from_binary_transform_algorithm,
                                      "FromBinaryTransformAlgorithm")
    if v.to_binary_transform_algorithm is not None:
        _w_ArgumentInputAlgorithmType(w, v.to_binary_transform_algorithm,
                                      "ToBinaryTransformAlgorithm")
    w.end(tag)


def _w_ArgumentBinaryDataType(w: X.Writer, v: ArgumentBinaryDataType,
                              tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentBinaryDataChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentBooleanDataType(w: X.Writer, v: ArgumentBooleanDataType,
                               tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.one_string_value is not None:
        w.attr('oneStringValue', v.one_string_value)
    if v.zero_string_value is not None:
        w.attr('zeroStringValue', v.zero_string_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentBooleanDataChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentBooleanExpressionType(
        w: X.Writer, v: ArgumentBooleanExpressionType, tag: str) -> None:
    w.start(tag)
    _w_ArgumentBooleanExpressionChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentComparisonCheckType(w: X.Writer, v: ArgumentComparisonCheckType,
                                   tag: str) -> None:
    w.start(tag)
    _w_ArgumentComparisonCheckChoice1(w, v.choice1)
    w.leaf('ComparisonOperator', v.comparison_operator.value)
    _w_ArgumentComparisonCheckChoice2(w, v.choice2)
    w.end(tag)


def _w_ArgumentComparisonListType(w: X.Writer, v: ArgumentComparisonListType,
                                  tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.comparison), 1, -1,
                  "ArgumentComparisonListType.Comparison")
    for e in v.comparison:
        _w_ArgumentComparisonType(w, e, 'Comparison')
    w.end(tag)


def _w_ArgumentComparisonType(w: X.Writer, v: ArgumentComparisonType,
                              tag: str) -> None:
    w.start(tag)
    if v.comparison_operator is not None:
        w.attr('comparisonOperator', v.comparison_operator.value)
    w.attr('value', v.value)
    _w_ArgumentComparisonChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentContainerRefEntryType(
        w: X.Writer, v: ArgumentContainerRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('containerRef', v.container_ref)
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentContainerSegmentRefEntryType(
        w: X.Writer, v: ArgumentContainerSegmentRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('containerRef', v.container_ref)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentDimensionListType(w: X.Writer, v: ArgumentDimensionListType,
                                 tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.dimension), 1, -1,
                  "ArgumentDimensionListType.Dimension")
    for e in v.dimension:
        _w_ArgumentDimensionType(w, e, 'Dimension')
    w.end(tag)


def _w_ArgumentDimensionType(w: X.Writer, v: ArgumentDimensionType,
                             tag: str) -> None:
    w.start(tag)
    _w_ArgumentIntegerValueType(w, v.starting_index, 'StartingIndex')
    _w_ArgumentIntegerValueType(w, v.ending_index, 'EndingIndex')
    w.end(tag)


def _w_ArgumentDiscreteLookupListType(
        w: X.Writer, v: ArgumentDiscreteLookupListType, tag: str) -> None:
    w.start(tag)
    w.attr('defaultValue', X.f_int(v.default_value))
    X.check_count(len(v.discrete_lookup), 1, -1,
                  "ArgumentDiscreteLookupListType.DiscreteLookup")
    for e in v.discrete_lookup:
        _w_ArgumentDiscreteLookupType(w, e, 'DiscreteLookup')
    w.end(tag)


def _w_ArgumentDiscreteLookupType(w: X.Writer, v: ArgumentDiscreteLookupType,
                                  tag: str) -> None:
    w.start(tag)
    w.attr('value', X.f_int(v.value))
    _w_ArgumentDiscreteLookupChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentDynamicValueType(w: X.Writer, v: ArgumentDynamicValueType,
                                tag: str) -> None:
    w.start(tag)
    _w_ArgumentDynamicValueChoice(w, v.choice)
    if v.linear_adjustment is not None:
        _w_LinearAdjustmentType(w, v.linear_adjustment, 'LinearAdjustment')
    w.end(tag)


def _w_ArgumentEnumeratedDataType(w: X.Writer, v: ArgumentEnumeratedDataType,
                                  tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentEnumeratedDataChoice(w, v.choice)
    _w_EnumerationListType(w, v.enumeration_list, 'EnumerationList')
    w.end(tag)


def _w_ArgumentFixedValueEntryType(w: X.Writer, v: ArgumentFixedValueEntryType,
                                   tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    w.attr('binaryValue', v.binary_value)
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentFloatDataType(w: X.Writer, v: ArgumentFloatDataType,
                             tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_double(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', v.size_in_bits.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentFloatDataChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    w.end(tag)


def _w_ArgumentIndirectParameterRefEntryType(
        w: X.Writer, v: ArgumentIndirectParameterRefEntryType,
        tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.alias_name_space is not None:
        w.attr('aliasNameSpace', v.alias_name_space)
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_ParameterInstanceRefType(w, v.parameter_instance, 'ParameterInstance')
    w.end(tag)


def _w_ArgumentInputAlgorithmType(w: X.Writer, v: ArgumentInputAlgorithmType,
                                  tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.algorithm_text is not None:
        _w_AlgorithmTextType(w, v.algorithm_text, 'AlgorithmText')
    if v.external_algorithm_set is not None:
        _w_ExternalAlgorithmSetType(w, v.external_algorithm_set,
                                    "ExternalAlgorithmSet")
    if v.input_set is not None:
        _w_ArgumentInputSetType(w, v.input_set, 'InputSet')
    w.end(tag)


def _w_ArgumentInputSetType(w: X.Writer, v: ArgumentInputSetType,
                            tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1,
                  "ArgumentInputSetType.ArgumentInputSetChoice")
    for e in v.items:
        _w_ArgumentInputSetChoice(w, e)
    w.end(tag)


def _w_ArgumentInstanceRefType(w: X.Writer, v: ArgumentInstanceRefType,
                               tag: str) -> None:
    w.start(tag)
    w.attr('argumentRef', v.argument_ref)
    if v.use_calibrated_value is not None:
        w.attr('useCalibratedValue', X.f_bool(v.use_calibrated_value))
    w.end(tag)


def _w_ArgumentIntegerDataType(w: X.Writer, v: ArgumentIntegerDataType,
                               tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_int(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.signed is not None:
        w.attr('signed', X.f_bool(v.signed))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentIntegerDataChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    w.end(tag)


def _w_ArgumentIntegerValueType(w: X.Writer, v: ArgumentIntegerValueType,
                                tag: str) -> None:
    w.start(tag)
    _w_ArgumentIntegerValueChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentListType(w: X.Writer, v: ArgumentListType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.argument), 1, -1, 'ArgumentListType.Argument')
    for e in v.argument:
        _w_ArgumentType(w, e, 'Argument')
    w.end(tag)


def _w_ArgumentLocationInContainerInBitsType(
        w: X.Writer, v: ArgumentLocationInContainerInBitsType,
        tag: str) -> None:
    w.start(tag)
    if v.reference_location is not None:
        w.attr('referenceLocation', v.reference_location.value)
    _w_ArgumentLocationInContainerInBitsChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentMatchCriteriaType(w: X.Writer, v: ArgumentMatchCriteriaType,
                                 tag: str) -> None:
    w.start(tag)
    _w_ArgumentMatchCriteriaChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentMathOperationType(w: X.Writer, v: ArgumentMathOperationType,
                                 tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1,
                  "ArgumentMathOperationType.ArgumentMathOperationChoice")
    for e in v.items:
        _w_ArgumentMathOperationChoice(w, e)
    w.end(tag)


def _w_ArgumentORedConditionsType(w: X.Writer, v: ArgumentORedConditionsType,
                                  tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 2, -1,
                  "ArgumentORedConditionsType.ArgumentORedConditionsChoice")
    for e in v.items:
        _w_ArgumentORedConditionsChoice(w, e)
    w.end(tag)


def _w_ArgumentParameterRefEntryType(
        w: X.Writer, v: ArgumentParameterRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('parameterRef', v.parameter_ref)
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentParameterSegmentRefEntryType(
        w: X.Writer, v: ArgumentParameterSegmentRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('parameterRef', v.parameter_ref)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentRelativeTimeDataType(
        w: X.Writer, v: ArgumentRelativeTimeDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_ArgumentRepeatType(w: X.Writer, v: ArgumentRepeatType, tag: str) -> None:
    w.start(tag)
    _w_ArgumentIntegerValueType(w, v.count, 'Count')
    if v.offset is not None:
        _w_ArgumentIntegerValueType(w, v.offset, 'Offset')
    w.end(tag)


def _w_ArgumentRestrictionListType(w: X.Writer, v: ArgumentRestrictionListType,
                                   tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.argument_restriction), 1, -1,
                  "ArgumentRestrictionListType.ArgumentRestriction")
    for e in v.argument_restriction:
        _w_ArgumentAssignmentType(w, e, 'ArgumentRestriction')
    w.end(tag)


def _w_ArgumentSequenceEntryType(w: X.Writer, v: ArgumentSequenceEntryType,
                                 tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentStreamSegmentEntryType(
        w: X.Writer, v: ArgumentStreamSegmentEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('streamRef', v.stream_ref)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_ArgumentLocationInContainerInBitsType(
            w, v.location_in_container_in_bits, "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_ArgumentRepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_ArgumentMatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentStringDataEncodingType(
        w: X.Writer, v: ArgumentStringDataEncodingType, tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.encoding is not None:
        w.attr('encoding', v.encoding.value)
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    _w_ArgumentStringDataEncodingChoice(w, v.choice)
    w.end(tag)


def _w_ArgumentStringDataType(w: X.Writer, v: ArgumentStringDataType,
                              tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.restriction_pattern is not None:
        w.attr('restrictionPattern', v.restriction_pattern)
    if v.character_width is not None:
        w.attr('characterWidth', v.character_width.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_ArgumentStringDataChoice(w, v.choice)
    if v.size_range_in_characters is not None:
        _w_IntegerRangeType(w, v.size_range_in_characters,
                            "SizeRangeInCharacters")
    w.end(tag)


def _w_ArgumentType(w: X.Writer, v: ArgumentType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    w.attr('argumentTypeRef', v.argument_type_ref)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArgumentTypeSetType(w: X.Writer, v: ArgumentTypeSetType,
                           tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1,
                  "ArgumentTypeSetType.ArgumentTypeSetChoice")
    for e in v.items:
        _w_ArgumentTypeSetChoice(w, e)
    w.end(tag)


def _w_ArgumentVariableStringType(w: X.Writer, v: ArgumentVariableStringType,
                                  tag: str) -> None:
    w.start(tag)
    w.attr('maxSizeInBits', X.f_int(v.max_size_in_bits))
    if v.choice1 is not None:
        _w_ArgumentVariableStringChoice1(w, v.choice1)
    _w_ArgumentVariableStringChoice2(w, v.choice2)
    w.end(tag)


def _w_ArrayArgumentType(w: X.Writer, v: ArrayArgumentType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    w.attr('arrayTypeRef', v.array_type_ref)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_ArgumentDimensionListType(w, v.dimension_list, 'DimensionList')
    w.end(tag)


def _w_ArrayDataTypeType(w: X.Writer, v: ArrayDataTypeType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    w.attr('arrayTypeRef', v.array_type_ref)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ArrayParameterRefEntryType(w: X.Writer, v: ArrayParameterRefEntryType,
                                  tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('parameterRef', v.parameter_ref)
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.dimension_list is not None:
        _w_DimensionListType(w, v.dimension_list, 'DimensionList')
    w.end(tag)


def _w_ArrayParameterType(w: X.Writer, v: ArrayParameterType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    w.attr('arrayTypeRef', v.array_type_ref)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_DimensionListType(w, v.dimension_list, 'DimensionList')
    w.end(tag)


def _w_AuthorSetType(w: X.Writer, v: AuthorSetType, tag: str) -> None:
    w.start(tag)
    for e in v.author:
        w.leaf('Author', e)
    w.end(tag)


def _w_AutoInvertType(w: X.Writer, v: AutoInvertType, tag: str) -> None:
    w.start(tag)
    if v.bad_frames_to_auto_invert is not None:
        w.attr('badFramesToAutoInvert', X.f_int(v.bad_frames_to_auto_invert))
    if v.invert_algorithm is not None:
        _w_InputAlgorithmType(w, v.invert_algorithm, 'InvertAlgorithm')
    w.end(tag)


def _w_BaseAlarmType(w: X.Writer, v: BaseAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_BaseCalibratorType(w: X.Writer, v: BaseCalibratorType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_BaseComparisonType(w: X.Writer, v: BaseComparisonType, tag: str) -> None:
    w.start(tag)
    w.end(tag)


def _w_BaseConditionsType(w: X.Writer, v: BaseConditionsType, tag: str) -> None:
    w.start(tag)
    w.end(tag)


def _w_BaseContainerType(w: X.Writer, v: BaseContainerType, tag: str) -> None:
    w.start(tag)
    w.attr('containerRef', v.container_ref)
    if v.restriction_criteria is not None:
        _w_RestrictionCriteriaType(w, v.restriction_criteria,
                                   "RestrictionCriteria")
    w.end(tag)


def _w_BaseDataType(w: X.Writer, v: BaseDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BaseDataChoice(w, v.choice)
    w.end(tag)


def _w_BaseMetaCommandType(w: X.Writer, v: BaseMetaCommandType,
                           tag: str) -> None:
    w.start(tag)
    w.attr('metaCommandRef', v.meta_command_ref)
    if v.argument_assignment_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_assignment_list,
                                      "ArgumentAssignmentList")
    w.end(tag)


def _w_BaseTimeDataType(w: X.Writer, v: BaseTimeDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_BaseTriggerType(w: X.Writer, v: BaseTriggerType, tag: str) -> None:
    w.start(tag)
    w.end(tag)


def _w_BinaryAlarmType(w: X.Writer, v: BinaryAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_BinaryAlarmChoice(w, v.choice)
    w.end(tag)


def _w_BinaryArgumentType(w: X.Writer, v: BinaryArgumentType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BinaryArgumentChoice(w, v.choice)
    w.end(tag)


def _w_BinaryContextAlarmListType(w: X.Writer, v: BinaryContextAlarmListType,
                                  tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_alarm), 1, -1,
                  "BinaryContextAlarmListType.ContextAlarm")
    for e in v.context_alarm:
        _w_BinaryContextAlarmType(w, e, 'ContextAlarm')
    w.end(tag)


def _w_BinaryContextAlarmType(w: X.Writer, v: BinaryContextAlarmType,
                              tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_BinaryContextAlarmChoice(w, v.choice)
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    w.end(tag)


def _w_BinaryDataEncodingType(w: X.Writer, v: BinaryDataEncodingType,
                              tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    _w_IntegerValueType(w, v.size_in_bits, 'SizeInBits')
    if v.from_binary_transform_algorithm is not None:
        _w_InputAlgorithmType(w, v.from_binary_transform_algorithm,
                              "FromBinaryTransformAlgorithm")
    if v.to_binary_transform_algorithm is not None:
        _w_InputAlgorithmType(w, v.to_binary_transform_algorithm,
                              "ToBinaryTransformAlgorithm")
    w.end(tag)


def _w_BinaryDataType(w: X.Writer, v: BinaryDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BinaryDataChoice(w, v.choice)
    w.end(tag)


def _w_BinaryParameterType(w: X.Writer, v: BinaryParameterType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BinaryParameterChoice(w, v.choice)
    if v.default_alarm is not None:
        _w_BinaryAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_BinaryContextAlarmListType(w, v.context_alarm_list,
                                      "ContextAlarmList")
    w.end(tag)


def _w_BlockMetaCommandType(w: X.Writer, v: BlockMetaCommandType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_MetaCommandStepListType(w, v.meta_command_step_list,
                               "MetaCommandStepList")
    w.end(tag)


def _w_BooleanAlarmType(w: X.Writer, v: BooleanAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_BooleanAlarmChoice(w, v.choice)
    w.end(tag)


def _w_BooleanArgumentType(w: X.Writer, v: BooleanArgumentType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.one_string_value is not None:
        w.attr('oneStringValue', v.one_string_value)
    if v.zero_string_value is not None:
        w.attr('zeroStringValue', v.zero_string_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BooleanArgumentChoice(w, v.choice)
    w.end(tag)


def _w_BooleanContextAlarmListType(w: X.Writer, v: BooleanContextAlarmListType,
                                   tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_alarm), 1, -1,
                  "BooleanContextAlarmListType.ContextAlarm")
    for e in v.context_alarm:
        _w_BooleanContextAlarmType(w, e, 'ContextAlarm')
    w.end(tag)


def _w_BooleanContextAlarmType(w: X.Writer, v: BooleanContextAlarmType,
                               tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_BooleanContextAlarmChoice(w, v.choice)
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    w.end(tag)


def _w_BooleanDataType(w: X.Writer, v: BooleanDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.one_string_value is not None:
        w.attr('oneStringValue', v.one_string_value)
    if v.zero_string_value is not None:
        w.attr('zeroStringValue', v.zero_string_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BooleanDataChoice(w, v.choice)
    w.end(tag)


def _w_BooleanExpressionType(w: X.Writer, v: BooleanExpressionType,
                             tag: str) -> None:
    w.start(tag)
    _w_BooleanExpressionChoice(w, v.choice)
    w.end(tag)


def _w_BooleanParameterType(w: X.Writer, v: BooleanParameterType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.one_string_value is not None:
        w.attr('oneStringValue', v.one_string_value)
    if v.zero_string_value is not None:
        w.attr('zeroStringValue', v.zero_string_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_BooleanParameterChoice(w, v.choice)
    if v.default_alarm is not None:
        _w_BooleanAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_BooleanContextAlarmListType(w, v.context_alarm_list,
                                       "ContextAlarmList")
    w.end(tag)


def _w_ByteType(w: X.Writer, v: ByteType, tag: str) -> None:
    w.start(tag)
    w.attr('byteSignificance', X.f_int(v.byte_significance))
    w.end(tag)


def _w_CRCType(w: X.Writer, v: CRCType, tag: str) -> None:
    w.start(tag)
    w.attr('width', X.f_int(v.width))
    if v.reflect_data is not None:
        w.attr('reflectData', X.f_bool(v.reflect_data))
    if v.reflect_remainder is not None:
        w.attr('reflectRemainder', X.f_bool(v.reflect_remainder))
    if v.direction is not None:
        w.attr('direction', v.direction.value)
    if v.bits_from_reference is not None:
        w.attr('bitsFromReference', X.f_int(v.bits_from_reference))
    if v.reference is not None:
        w.attr('reference', v.reference.value)
    if v.parameter_ref is not None:
        w.attr('parameterRef', v.parameter_ref)
    w.leaf('Polynomial', v.polynomial)
    if v.init_remainder is not None:
        w.leaf('InitRemainder', v.init_remainder)
    if v.final_xor is not None:
        w.leaf('FinalXOR', v.final_xor)
    w.end(tag)


def _w_CalibratorType(w: X.Writer, v: CalibratorType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_CalibratorChoice(w, v.choice)
    w.end(tag)


def _w_ChangeAlarmRangesType(w: X.Writer, v: ChangeAlarmRangesType,
                             tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.range_form is not None:
        w.attr('rangeForm', v.range_form.value)
    if v.change_type is not None:
        w.attr('changeType', v.change_type.value)
    if v.change_basis is not None:
        w.attr('changeBasis', v.change_basis.value)
    if v.span_of_interest_in_samples is not None:
        w.attr("spanOfInterestInSamples",
               X.f_int(v.span_of_interest_in_samples))
    if v.span_of_interest_in_seconds is not None:
        w.attr("spanOfInterestInSeconds",
               X.f_double(v.span_of_interest_in_seconds))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.watch_range is not None:
        _w_FloatRangeType(w, v.watch_range, 'WatchRange')
    if v.warning_range is not None:
        _w_FloatRangeType(w, v.warning_range, 'WarningRange')
    if v.distress_range is not None:
        _w_FloatRangeType(w, v.distress_range, 'DistressRange')
    if v.critical_range is not None:
        _w_FloatRangeType(w, v.critical_range, 'CriticalRange')
    if v.severe_range is not None:
        _w_FloatRangeType(w, v.severe_range, 'SevereRange')
    w.end(tag)


def _w_ChangeValueType(w: X.Writer, v: ChangeValueType, tag: str) -> None:
    w.start(tag)
    w.attr('value', X.f_double(v.value))
    w.end(tag)


def _w_CheckWindowAlgorithmsType(w: X.Writer, v: CheckWindowAlgorithmsType,
                                 tag: str) -> None:
    w.start(tag)
    _w_InputAlgorithmType(w, v.start_check, 'StartCheck')
    _w_InputAlgorithmType(w, v.stop_time, 'StopTime')
    w.end(tag)


def _w_CheckWindowType(w: X.Writer, v: CheckWindowType, tag: str) -> None:
    w.start(tag)
    if v.time_to_start_checking is not None:
        w.attr('timeToStartChecking', v.time_to_start_checking)
    w.attr('timeToStopChecking', v.time_to_stop_checking)
    if v.time_window_is_relative_to is not None:
        w.attr('timeWindowIsRelativeTo', v.time_window_is_relative_to.value)
    w.end(tag)


def _w_ChecksumType(w: X.Writer, v: ChecksumType, tag: str) -> None:
    w.start(tag)
    if v.bits_from_reference is not None:
        w.attr('bitsFromReference', X.f_int(v.bits_from_reference))
    if v.reference is not None:
        w.attr('reference', v.reference.value)
    w.attr('name', v.name.value)
    if v.hash_size_in_bits is not None:
        w.attr('hashSizeInBits', X.f_int(v.hash_size_in_bits))
    if v.parameter_ref is not None:
        w.attr('parameterRef', v.parameter_ref)
    if v.input_algorithm is not None:
        _w_InputAlgorithmType(w, v.input_algorithm, 'InputAlgorithm')
    w.end(tag)


def _w_CommandContainerEntryListType(
        w: X.Writer, v: CommandContainerEntryListType, tag: str) -> None:
    w.start(tag)
    for e in v.items:
        _w_CommandContainerEntryListChoice(w, e)
    w.end(tag)


def _w_CommandContainerSetType(w: X.Writer, v: CommandContainerSetType,
                               tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.command_container), 1, -1,
                  "CommandContainerSetType.CommandContainer")
    for e in v.command_container:
        _w_SequenceContainerType(w, e, 'CommandContainer')
    w.end(tag)


def _w_CommandContainerType(w: X.Writer, v: CommandContainerType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.default_rate_in_stream is not None:
        _w_RateInStreamType(w, v.default_rate_in_stream, 'DefaultRateInStream')
    if v.rate_in_stream_set is not None:
        _w_RateInStreamSetType(w, v.rate_in_stream_set, 'RateInStreamSet')
    if v.binary_encoding is not None:
        _w_ContainerBinaryDataEncodingType(w, v.binary_encoding,
                                           "BinaryEncoding")
    _w_CommandContainerEntryListType(w, v.entry_list, 'EntryList')
    if v.base_container is not None:
        _w_BaseContainerType(w, v.base_container, 'BaseContainer')
    w.end(tag)


def _w_CommandMetaDataType(w: X.Writer, v: CommandMetaDataType,
                           tag: str) -> None:
    w.start(tag)
    if v.parameter_type_set is not None:
        _w_ParameterTypeSetType(w, v.parameter_type_set, 'ParameterTypeSet')
    if v.parameter_set is not None:
        _w_ParameterSetType(w, v.parameter_set, 'ParameterSet')
    if v.argument_type_set is not None:
        _w_ArgumentTypeSetType(w, v.argument_type_set, 'ArgumentTypeSet')
    if v.meta_command_set is not None:
        _w_MetaCommandSetType(w, v.meta_command_set, 'MetaCommandSet')
    if v.command_container_set is not None:
        _w_CommandContainerSetType(w, v.command_container_set,
                                   "CommandContainerSet")
    if v.stream_set is not None:
        _w_StreamSetType(w, v.stream_set, 'StreamSet')
    if v.algorithm_set is not None:
        _w_AlgorithmSetType(w, v.algorithm_set, 'AlgorithmSet')
    w.end(tag)


def _w_CommandVerifierType(w: X.Writer, v: CommandVerifierType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_CommandVerifierChoice1(w, v.choice1)
    _w_CommandVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_ComparisonCheckType(w: X.Writer, v: ComparisonCheckType,
                           tag: str) -> None:
    w.start(tag)
    _w_ParameterInstanceRefType(w, v.parameter_instance_ref,
                                "ParameterInstanceRef")
    w.leaf('ComparisonOperator', v.comparison_operator.value)
    _w_ComparisonCheckChoice(w, v.choice)
    w.end(tag)


def _w_ComparisonListType(w: X.Writer, v: ComparisonListType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.comparison), 1, -1, 'ComparisonListType.Comparison')
    for e in v.comparison:
        _w_ComparisonType(w, e, 'Comparison')
    w.end(tag)


def _w_ComparisonType(w: X.Writer, v: ComparisonType, tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    if v.instance is not None:
        w.attr('instance', X.f_int(v.instance))
    if v.use_calibrated_value is not None:
        w.attr('useCalibratedValue', X.f_bool(v.use_calibrated_value))
    if v.comparison_operator is not None:
        w.attr('comparisonOperator', v.comparison_operator.value)
    w.attr('value', v.value)
    w.end(tag)


def _w_CompleteVerifierType(w: X.Writer, v: CompleteVerifierType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_CompleteVerifierChoice1(w, v.choice1)
    _w_CompleteVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    if v.return_parm_ref is not None:
        _w_ParameterRefType(w, v.return_parm_ref, 'ReturnParmRef')
    w.end(tag)


def _w_ConstantType(w: X.Writer, v: ConstantType, tag: str) -> None:
    w.start(tag)
    w.attr('constantName', v.constant_name)
    w.attr('value', v.value)
    w.end(tag)


def _w_ContainerBinaryDataEncodingType(
        w: X.Writer, v: ContainerBinaryDataEncodingType, tag: str) -> None:
    w.start(tag)
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    if v.size_in_bits is not None:
        _w_IntegerValueType(w, v.size_in_bits, 'SizeInBits')
    if v.from_binary_transform_algorithm is not None:
        _w_InputAlgorithmType(w, v.from_binary_transform_algorithm,
                              "FromBinaryTransformAlgorithm")
    if v.to_binary_transform_algorithm is not None:
        _w_InputAlgorithmType(w, v.to_binary_transform_algorithm,
                              "ToBinaryTransformAlgorithm")
    w.end(tag)


def _w_ContainerRefEntryType(w: X.Writer, v: ContainerRefEntryType,
                             tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('containerRef', v.container_ref)
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ContainerRefSetType(w: X.Writer, v: ContainerRefSetType,
                           tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.container_ref), 1, -1,
                  "ContainerRefSetType.ContainerRef")
    for e in v.container_ref:
        _w_ContainerRefType(w, e, 'ContainerRef')
    w.end(tag)


def _w_ContainerRefType(w: X.Writer, v: ContainerRefType, tag: str) -> None:
    w.start(tag)
    w.attr('containerRef', v.container_ref)
    w.end(tag)


def _w_ContainerSegmentRefEntryType(
        w: X.Writer, v: ContainerSegmentRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('containerRef', v.container_ref)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ContainerSetType(w: X.Writer, v: ContainerSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.sequence_container), 1, -1,
                  "ContainerSetType.SequenceContainer")
    for e in v.sequence_container:
        _w_SequenceContainerType(w, e, 'SequenceContainer')
    w.end(tag)


def _w_ContainerType(w: X.Writer, v: ContainerType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.default_rate_in_stream is not None:
        _w_RateInStreamType(w, v.default_rate_in_stream, 'DefaultRateInStream')
    if v.rate_in_stream_set is not None:
        _w_RateInStreamSetType(w, v.rate_in_stream_set, 'RateInStreamSet')
    if v.binary_encoding is not None:
        _w_ContainerBinaryDataEncodingType(w, v.binary_encoding,
                                           "BinaryEncoding")
    w.end(tag)


def _w_ContextCalibratorListType(w: X.Writer, v: ContextCalibratorListType,
                                 tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_calibrator), 1, -1,
                  "ContextCalibratorListType.ContextCalibrator")
    for e in v.context_calibrator:
        _w_ContextCalibratorType(w, e, 'ContextCalibrator')
    w.end(tag)


def _w_ContextCalibratorType(w: X.Writer, v: ContextCalibratorType,
                             tag: str) -> None:
    w.start(tag)
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    _w_CalibratorType(w, v.calibrator, 'Calibrator')
    w.end(tag)


def _w_ContextMatchType(w: X.Writer, v: ContextMatchType, tag: str) -> None:
    w.start(tag)
    _w_ContextMatchChoice(w, v.choice)
    w.end(tag)


def _w_ContextSignificanceListType(w: X.Writer, v: ContextSignificanceListType,
                                   tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_significance), 1, -1,
                  "ContextSignificanceListType.ContextSignificance")
    for e in v.context_significance:
        _w_ContextSignificanceType(w, e, 'ContextSignificance')
    w.end(tag)


def _w_ContextSignificanceType(w: X.Writer, v: ContextSignificanceType,
                               tag: str) -> None:
    w.start(tag)
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    _w_SignificanceType(w, v.significance, 'Significance')
    w.end(tag)


def _w_CustomAlarmType(w: X.Writer, v: CustomAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_InputAlgorithmType(w, v.input_algorithm, 'InputAlgorithm')
    w.end(tag)


def _w_CustomStreamType(w: X.Writer, v: CustomStreamType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.bit_rate_in_bps is not None:
        w.attr('bitRateInBPS', X.f_double(v.bit_rate_in_bps))
    if v.pcm_type is not None:
        w.attr('pcmType', v.pcm_type.value)
    if v.inverted is not None:
        w.attr('inverted', X.f_bool(v.inverted))
    w.attr('encodedStreamRef', v.encoded_stream_ref)
    w.attr('decodedStreamRef', v.decoded_stream_ref)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_InputAlgorithmType(w, v.encoding_algorithm, 'EncodingAlgorithm')
    _w_InputOutputAlgorithmType(w, v.decoding_algorithm, 'DecodingAlgorithm')
    w.end(tag)


def _w_DataEncodingType(w: X.Writer, v: DataEncodingType, tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    w.end(tag)


def _w_DescriptionType(w: X.Writer, v: DescriptionType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_DimensionListType(w: X.Writer, v: DimensionListType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.dimension), 1, -1, 'DimensionListType.Dimension')
    for e in v.dimension:
        _w_DimensionType(w, e, 'Dimension')
    w.end(tag)


def _w_DimensionType(w: X.Writer, v: DimensionType, tag: str) -> None:
    w.start(tag)
    _w_IntegerValueType(w, v.starting_index, 'StartingIndex')
    _w_IntegerValueType(w, v.ending_index, 'EndingIndex')
    w.end(tag)


def _w_DiscreteLookupListType(w: X.Writer, v: DiscreteLookupListType,
                              tag: str) -> None:
    w.start(tag)
    w.attr('defaultValue', X.f_int(v.default_value))
    X.check_count(len(v.discrete_lookup), 1, -1,
                  "DiscreteLookupListType.DiscreteLookup")
    for e in v.discrete_lookup:
        _w_DiscreteLookupType(w, e, 'DiscreteLookup')
    w.end(tag)


def _w_DiscreteLookupType(w: X.Writer, v: DiscreteLookupType, tag: str) -> None:
    w.start(tag)
    w.attr('value', X.f_int(v.value))
    _w_DiscreteLookupChoice(w, v.choice)
    w.end(tag)


def _w_DynamicValueType(w: X.Writer, v: DynamicValueType, tag: str) -> None:
    w.start(tag)
    _w_ParameterInstanceRefType(w, v.parameter_instance_ref,
                                "ParameterInstanceRef")
    if v.linear_adjustment is not None:
        _w_LinearAdjustmentType(w, v.linear_adjustment, 'LinearAdjustment')
    w.end(tag)


def _w_EncodingType(w: X.Writer, v: EncodingType, tag: str) -> None:
    w.start(tag)
    if v.units is not None:
        w.attr('units', v.units.value)
    if v.scale is not None:
        w.attr('scale', X.f_double(v.scale))
    if v.offset is not None:
        w.attr('offset', X.f_double(v.offset))
    _w_EncodingChoice(w, v.choice)
    w.end(tag)


def _w_EntryListType(w: X.Writer, v: EntryListType, tag: str) -> None:
    w.start(tag)
    for e in v.items:
        _w_EntryListChoice(w, e)
    w.end(tag)


def _w_EnumeratedArgumentType(w: X.Writer, v: EnumeratedArgumentType,
                              tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_EnumeratedArgumentChoice(w, v.choice)
    _w_EnumerationListType(w, v.enumeration_list, 'EnumerationList')
    w.end(tag)


def _w_EnumeratedDataType(w: X.Writer, v: EnumeratedDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_EnumeratedDataChoice(w, v.choice)
    _w_EnumerationListType(w, v.enumeration_list, 'EnumerationList')
    w.end(tag)


def _w_EnumeratedParameterType(w: X.Writer, v: EnumeratedParameterType,
                               tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_EnumeratedParameterChoice(w, v.choice)
    _w_EnumerationListType(w, v.enumeration_list, 'EnumerationList')
    if v.default_alarm is not None:
        _w_EnumerationAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_EnumerationContextAlarmListType(w, v.context_alarm_list,
                                           "ContextAlarmList")
    w.end(tag)


def _w_EnumerationAlarmLevelType(w: X.Writer, v: EnumerationAlarmLevelType,
                                 tag: str) -> None:
    w.start(tag)
    w.attr('alarmLevel', v.alarm_level.value)
    w.attr('enumerationLabel', v.enumeration_label)
    w.end(tag)


def _w_EnumerationAlarmListType(w: X.Writer, v: EnumerationAlarmListType,
                                tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.enumeration_alarm), 1, -1,
                  "EnumerationAlarmListType.EnumerationAlarm")
    for e in v.enumeration_alarm:
        _w_EnumerationAlarmLevelType(w, e, 'EnumerationAlarm')
    w.end(tag)


def _w_EnumerationAlarmType(w: X.Writer, v: EnumerationAlarmType,
                            tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.default_alarm_level is not None:
        w.attr('defaultAlarmLevel', v.default_alarm_level.value)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_EnumerationAlarmChoice(w, v.choice)
    if v.enumeration_alarm_list is not None:
        _w_EnumerationAlarmListType(w, v.enumeration_alarm_list,
                                    "EnumerationAlarmList")
    w.end(tag)


def _w_EnumerationContextAlarmListType(
        w: X.Writer, v: EnumerationContextAlarmListType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_alarm), 1, -1,
                  "EnumerationContextAlarmListType.ContextAlarm")
    for e in v.context_alarm:
        _w_EnumerationContextAlarmType(w, e, 'ContextAlarm')
    w.end(tag)


def _w_EnumerationContextAlarmType(w: X.Writer, v: EnumerationContextAlarmType,
                                   tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.default_alarm_level is not None:
        w.attr('defaultAlarmLevel', v.default_alarm_level.value)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_EnumerationContextAlarmChoice(w, v.choice)
    if v.enumeration_alarm_list is not None:
        _w_EnumerationAlarmListType(w, v.enumeration_alarm_list,
                                    "EnumerationAlarmList")
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    w.end(tag)


def _w_EnumerationListType(w: X.Writer, v: EnumerationListType,
                           tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.enumeration), 1, -1, 'EnumerationListType.Enumeration')
    for e in v.enumeration:
        _w_ValueEnumerationType(w, e, 'Enumeration')
    w.end(tag)


def _w_ErrorDetectCorrectType(w: X.Writer, v: ErrorDetectCorrectType,
                              tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1,
                  "ErrorDetectCorrectType.ErrorDetectCorrectChoice")
    for e in v.items:
        _w_ErrorDetectCorrectChoice(w, e)
    w.end(tag)


def _w_ExecutionVerifierType(w: X.Writer, v: ExecutionVerifierType,
                             tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_ExecutionVerifierChoice1(w, v.choice1)
    _w_ExecutionVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    if v.percent_complete is not None:
        _w_PercentCompleteType(w, v.percent_complete, 'PercentComplete')
    w.end(tag)


def _w_ExternalAlgorithmSetType(w: X.Writer, v: ExternalAlgorithmSetType,
                                tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.external_algorithm), 1, -1,
                  "ExternalAlgorithmSetType.ExternalAlgorithm")
    for e in v.external_algorithm:
        _w_ExternalAlgorithmType(w, e, 'ExternalAlgorithm')
    w.end(tag)


def _w_ExternalAlgorithmType(w: X.Writer, v: ExternalAlgorithmType,
                             tag: str) -> None:
    w.start(tag)
    w.attr('implementationName', v.implementation_name)
    w.attr('algorithmLocation', v.algorithm_location)
    w.end(tag)


def _w_FailedVerifierType(w: X.Writer, v: FailedVerifierType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_FailedVerifierChoice1(w, v.choice1)
    _w_FailedVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    if v.return_parm_ref is not None:
        _w_ParameterRefType(w, v.return_parm_ref, 'ReturnParmRef')
    w.end(tag)


def _w_FixedFrameStreamType(w: X.Writer, v: FixedFrameStreamType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.bit_rate_in_bps is not None:
        w.attr('bitRateInBPS', X.f_double(v.bit_rate_in_bps))
    if v.pcm_type is not None:
        w.attr('pcmType', v.pcm_type.value)
    if v.inverted is not None:
        w.attr('inverted', X.f_bool(v.inverted))
    if v.sync_aperture_in_bits is not None:
        w.attr('syncApertureInBits', X.f_int(v.sync_aperture_in_bits))
    w.attr('frameLengthInBits', X.f_int(v.frame_length_in_bits))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_FixedFrameStreamChoice(w, v.choice)
    if v.stream_ref is not None:
        _w_StreamRefType(w, v.stream_ref, 'StreamRef')
    _w_FixedFrameSyncStrategyType(w, v.sync_strategy, 'SyncStrategy')
    w.end(tag)


def _w_FixedFrameSyncStrategyType(w: X.Writer, v: FixedFrameSyncStrategyType,
                                  tag: str) -> None:
    w.start(tag)
    if v.verify_to_lock_good_frames is not None:
        w.attr('verifyToLockGoodFrames', X.f_int(v.verify_to_lock_good_frames))
    if v.check_to_lock_good_frames is not None:
        w.attr('checkToLockGoodFrames', X.f_int(v.check_to_lock_good_frames))
    if v.max_bit_errors_in_sync_pattern is not None:
        w.attr("maxBitErrorsInSyncPattern",
               X.f_int(v.max_bit_errors_in_sync_pattern))
    if v.auto_invert is not None:
        _w_AutoInvertType(w, v.auto_invert, 'AutoInvert')
    _w_SyncPatternType(w, v.sync_pattern, 'SyncPattern')
    w.end(tag)


def _w_FlagType(w: X.Writer, v: FlagType, tag: str) -> None:
    w.start(tag)
    if v.flag_size_in_bits is not None:
        w.attr('flagSizeInBits', X.f_int(v.flag_size_in_bits))
    if v.flag_bit_type is not None:
        w.attr('flagBitType', v.flag_bit_type.value)
    w.end(tag)


def _w_FloatArgumentType(w: X.Writer, v: FloatArgumentType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_double(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', v.size_in_bits.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_FloatArgumentChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    if v.valid_range_set is not None:
        _w_ValidFloatRangeSetType(w, v.valid_range_set, 'ValidRangeSet')
    w.end(tag)


def _w_FloatDataEncodingType(w: X.Writer, v: FloatDataEncodingType,
                             tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.encoding is not None:
        w.attr('encoding', v.encoding.value)
    if v.size_in_bits is not None:
        w.attr('sizeInBits', v.size_in_bits.value)
    if v.change_threshold is not None:
        w.attr('changeThreshold', X.f_double(v.change_threshold))
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    if v.default_calibrator is not None:
        _w_CalibratorType(w, v.default_calibrator, 'DefaultCalibrator')
    if v.context_calibrator_list is not None:
        _w_ContextCalibratorListType(w, v.context_calibrator_list,
                                     "ContextCalibratorList")
    w.end(tag)


def _w_FloatDataType(w: X.Writer, v: FloatDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_double(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', v.size_in_bits.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_FloatDataChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    if v.valid_range is not None:
        _w_FloatDataValidRangeType(w, v.valid_range, 'ValidRange')
    w.end(tag)


def _w_FloatDataValidRangeType(w: X.Writer, v: FloatDataValidRangeType,
                               tag: str) -> None:
    w.start(tag)
    if v.min_inclusive is not None:
        w.attr('minInclusive', X.f_double(v.min_inclusive))
    if v.min_exclusive is not None:
        w.attr('minExclusive', X.f_double(v.min_exclusive))
    if v.max_inclusive is not None:
        w.attr('maxInclusive', X.f_double(v.max_inclusive))
    if v.max_exclusive is not None:
        w.attr('maxExclusive', X.f_double(v.max_exclusive))
    if v.valid_range_applies_to_calibrated is not None:
        w.attr("validRangeAppliesToCalibrated",
               X.f_bool(v.valid_range_applies_to_calibrated))
    w.end(tag)


def _w_FloatParameterType(w: X.Writer, v: FloatParameterType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_double(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', v.size_in_bits.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_FloatParameterChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    if v.valid_range is not None:
        _w_FloatDataValidRangeType(w, v.valid_range, 'ValidRange')
    if v.default_alarm is not None:
        _w_NumericAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_NumericContextAlarmListType(w, v.context_alarm_list,
                                       "ContextAlarmList")
    w.end(tag)


def _w_FloatRangeType(w: X.Writer, v: FloatRangeType, tag: str) -> None:
    w.start(tag)
    if v.min_inclusive is not None:
        w.attr('minInclusive', X.f_double(v.min_inclusive))
    if v.min_exclusive is not None:
        w.attr('minExclusive', X.f_double(v.min_exclusive))
    if v.max_inclusive is not None:
        w.attr('maxInclusive', X.f_double(v.max_inclusive))
    if v.max_exclusive is not None:
        w.attr('maxExclusive', X.f_double(v.max_exclusive))
    w.end(tag)


def _w_FrameStreamType(w: X.Writer, v: FrameStreamType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.bit_rate_in_bps is not None:
        w.attr('bitRateInBPS', X.f_double(v.bit_rate_in_bps))
    if v.pcm_type is not None:
        w.attr('pcmType', v.pcm_type.value)
    if v.inverted is not None:
        w.attr('inverted', X.f_bool(v.inverted))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_FrameStreamChoice(w, v.choice)
    if v.stream_ref is not None:
        _w_StreamRefType(w, v.stream_ref, 'StreamRef')
    w.end(tag)


def _w_HeaderType(w: X.Writer, v: HeaderType, tag: str) -> None:
    w.start(tag)
    if v.version is not None:
        w.attr('version', v.version)
    if v.date is not None:
        w.attr('date', v.date)
    if v.classification is not None:
        w.attr('classification', v.classification)
    if v.classification_instructions is not None:
        w.attr('classificationInstructions', v.classification_instructions)
    w.attr('validationStatus', v.validation_status.value)
    if v.author_set is not None:
        _w_AuthorSetType(w, v.author_set, 'AuthorSet')
    if v.note_set is not None:
        _w_NoteSetType(w, v.note_set, 'NoteSet')
    if v.history_set is not None:
        _w_HistorySetType(w, v.history_set, 'HistorySet')
    w.end(tag)


def _w_HistorySetType(w: X.Writer, v: HistorySetType, tag: str) -> None:
    w.start(tag)
    for e in v.history:
        w.leaf('History', e)
    w.end(tag)


def _w_IndirectParameterRefEntryType(
        w: X.Writer, v: IndirectParameterRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.alias_name_space is not None:
        w.attr('aliasNameSpace', v.alias_name_space)
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_ParameterInstanceRefType(w, v.parameter_instance, 'ParameterInstance')
    w.end(tag)


def _w_InputAlgorithmType(w: X.Writer, v: InputAlgorithmType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.algorithm_text is not None:
        _w_AlgorithmTextType(w, v.algorithm_text, 'AlgorithmText')
    if v.external_algorithm_set is not None:
        _w_ExternalAlgorithmSetType(w, v.external_algorithm_set,
                                    "ExternalAlgorithmSet")
    if v.input_set is not None:
        _w_InputSetType(w, v.input_set, 'InputSet')
    w.end(tag)


def _w_InputOutputAlgorithmType(w: X.Writer, v: InputOutputAlgorithmType,
                                tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.thread is not None:
        w.attr('thread', X.f_bool(v.thread))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.algorithm_text is not None:
        _w_AlgorithmTextType(w, v.algorithm_text, 'AlgorithmText')
    if v.external_algorithm_set is not None:
        _w_ExternalAlgorithmSetType(w, v.external_algorithm_set,
                                    "ExternalAlgorithmSet")
    if v.input_set is not None:
        _w_InputSetType(w, v.input_set, 'InputSet')
    if v.output_set is not None:
        _w_OutputSetType(w, v.output_set, 'OutputSet')
    w.end(tag)


def _w_InputOutputTriggerAlgorithmType(
        w: X.Writer, v: InputOutputTriggerAlgorithmType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.thread is not None:
        w.attr('thread', X.f_bool(v.thread))
    if v.trigger_container is not None:
        w.attr('triggerContainer', v.trigger_container)
    if v.priority is not None:
        w.attr('priority', X.f_int(v.priority))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.algorithm_text is not None:
        _w_AlgorithmTextType(w, v.algorithm_text, 'AlgorithmText')
    if v.external_algorithm_set is not None:
        _w_ExternalAlgorithmSetType(w, v.external_algorithm_set,
                                    "ExternalAlgorithmSet")
    if v.input_set is not None:
        _w_InputSetType(w, v.input_set, 'InputSet')
    if v.output_set is not None:
        _w_OutputSetType(w, v.output_set, 'OutputSet')
    if v.trigger_set is not None:
        _w_TriggerSetType(w, v.trigger_set, 'TriggerSet')
    w.end(tag)


def _w_InputParameterInstanceRefType(
        w: X.Writer, v: InputParameterInstanceRefType, tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    if v.instance is not None:
        w.attr('instance', X.f_int(v.instance))
    if v.use_calibrated_value is not None:
        w.attr('useCalibratedValue', X.f_bool(v.use_calibrated_value))
    if v.input_name is not None:
        w.attr('inputName', v.input_name)
    w.end(tag)


def _w_InputSetType(w: X.Writer, v: InputSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1, 'InputSetType.InputSetChoice')
    for e in v.items:
        _w_InputSetChoice(w, e)
    w.end(tag)


def _w_IntegerArgumentType(w: X.Writer, v: IntegerArgumentType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_int(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.signed is not None:
        w.attr('signed', X.f_bool(v.signed))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_IntegerArgumentChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    if v.valid_range_set is not None:
        _w_ValidIntegerRangeSetType(w, v.valid_range_set, 'ValidRangeSet')
    w.end(tag)


def _w_IntegerDataEncodingType(w: X.Writer, v: IntegerDataEncodingType,
                               tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.encoding is not None:
        w.attr('encoding', v.encoding.value)
    if v.size_in_bits is not None:
        w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.change_threshold is not None:
        w.attr('changeThreshold', X.f_int(v.change_threshold))
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    if v.default_calibrator is not None:
        _w_CalibratorType(w, v.default_calibrator, 'DefaultCalibrator')
    if v.context_calibrator_list is not None:
        _w_ContextCalibratorListType(w, v.context_calibrator_list,
                                     "ContextCalibratorList")
    w.end(tag)


def _w_IntegerDataType(w: X.Writer, v: IntegerDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_int(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.signed is not None:
        w.attr('signed', X.f_bool(v.signed))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_IntegerDataChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    if v.valid_range is not None:
        _w_IntegerDataValidRangeType(w, v.valid_range, 'ValidRange')
    w.end(tag)


def _w_IntegerDataValidRangeType(w: X.Writer, v: IntegerDataValidRangeType,
                                 tag: str) -> None:
    w.start(tag)
    if v.min_inclusive is not None:
        w.attr('minInclusive', X.f_int(v.min_inclusive))
    if v.max_inclusive is not None:
        w.attr('maxInclusive', X.f_int(v.max_inclusive))
    if v.valid_range_applies_to_calibrated is not None:
        w.attr("validRangeAppliesToCalibrated",
               X.f_bool(v.valid_range_applies_to_calibrated))
    w.end(tag)


def _w_IntegerParameterType(w: X.Writer, v: IntegerParameterType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', X.f_int(v.initial_value))
    if v.size_in_bits is not None:
        w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.signed is not None:
        w.attr('signed', X.f_bool(v.signed))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_IntegerParameterChoice(w, v.choice)
    if v.to_string is not None:
        _w_ToStringType(w, v.to_string, 'ToString')
    if v.valid_range is not None:
        _w_IntegerDataValidRangeType(w, v.valid_range, 'ValidRange')
    if v.default_alarm is not None:
        _w_NumericAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_NumericContextAlarmListType(w, v.context_alarm_list,
                                       "ContextAlarmList")
    w.end(tag)


def _w_IntegerRangeType(w: X.Writer, v: IntegerRangeType, tag: str) -> None:
    w.start(tag)
    if v.min_inclusive is not None:
        w.attr('minInclusive', X.f_int(v.min_inclusive))
    if v.max_inclusive is not None:
        w.attr('maxInclusive', X.f_int(v.max_inclusive))
    w.end(tag)


def _w_IntegerValueType(w: X.Writer, v: IntegerValueType, tag: str) -> None:
    w.start(tag)
    _w_IntegerValueChoice(w, v.choice)
    w.end(tag)


def _w_InterlockType(w: X.Writer, v: InterlockType, tag: str) -> None:
    w.start(tag)
    if v.scope_to_space_system is not None:
        w.attr('scopeToSpaceSystem', v.scope_to_space_system)
    if v.verification_to_wait_for is not None:
        w.attr('verificationToWaitFor', v.verification_to_wait_for.value)
    if v.verification_progress_percentage is not None:
        w.attr("verificationProgressPercentage",
               X.f_double(v.verification_progress_percentage))
    if v.suspendable is not None:
        w.attr('suspendable', X.f_bool(v.suspendable))
    w.end(tag)


def _w_LeadingSizeType(w: X.Writer, v: LeadingSizeType, tag: str) -> None:
    w.start(tag)
    if v.size_in_bits_of_size_tag is not None:
        w.attr('sizeInBitsOfSizeTag', X.f_int(v.size_in_bits_of_size_tag))
    w.end(tag)


def _w_LinearAdjustmentType(w: X.Writer, v: LinearAdjustmentType,
                            tag: str) -> None:
    w.start(tag)
    if v.slope is not None:
        w.attr('slope', X.f_double(v.slope))
    if v.intercept is not None:
        w.attr('intercept', X.f_double(v.intercept))
    w.end(tag)


def _w_LocationInContainerInBitsType(
        w: X.Writer, v: LocationInContainerInBitsType, tag: str) -> None:
    w.start(tag)
    if v.reference_location is not None:
        w.attr('referenceLocation', v.reference_location.value)
    _w_LocationInContainerInBitsChoice(w, v.choice)
    w.end(tag)


def _w_MatchCriteriaType(w: X.Writer, v: MatchCriteriaType, tag: str) -> None:
    w.start(tag)
    _w_MatchCriteriaChoice(w, v.choice)
    w.end(tag)


def _w_MathAlgorithmType(w: X.Writer, v: MathAlgorithmType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_TriggeredMathOperationType(w, v.math_operation, 'MathOperation')
    w.end(tag)


def _w_MathOperationCalibratorType(w: X.Writer, v: MathOperationCalibratorType,
                                   tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.items), 1, -1,
                  "MathOperationCalibratorType.MathOperationCalibratorChoice")
    for e in v.items:
        _w_MathOperationCalibratorChoice(w, e)
    w.end(tag)


def _w_MathOperationType(w: X.Writer, v: MathOperationType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.items), 1, -1, 'MathOperationType.MathOperationChoice')
    for e in v.items:
        _w_MathOperationChoice(w, e)
    w.end(tag)


def _w_MemberListType(w: X.Writer, v: MemberListType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.member), 1, -1, 'MemberListType.Member')
    for e in v.member:
        _w_MemberType(w, e, 'Member')
    w.end(tag)


def _w_MemberType(w: X.Writer, v: MemberType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    w.attr('typeRef', v.type_ref)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_MessageRefSetType(w: X.Writer, v: MessageRefSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.message_ref), 1, -1, 'MessageRefSetType.MessageRef')
    for e in v.message_ref:
        _w_MessageRefType(w, e, 'MessageRef')
    w.end(tag)


def _w_MessageRefType(w: X.Writer, v: MessageRefType, tag: str) -> None:
    w.start(tag)
    w.attr('messageRef', v.message_ref)
    w.end(tag)


def _w_MessageSetType(w: X.Writer, v: MessageSetType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.message), 1, -1, 'MessageSetType.Message')
    for e in v.message:
        _w_MessageType(w, e, 'Message')
    w.end(tag)


def _w_MessageType(w: X.Writer, v: MessageType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_MatchCriteriaType(w, v.match_criteria, 'MatchCriteria')
    _w_ContainerRefType(w, v.container_ref, 'ContainerRef')
    w.end(tag)


def _w_MetaCommandSetType(w: X.Writer, v: MetaCommandSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1,
                  "MetaCommandSetType.MetaCommandSetChoice")
    for e in v.items:
        _w_MetaCommandSetChoice(w, e)
    w.end(tag)


def _w_MetaCommandStepListType(w: X.Writer, v: MetaCommandStepListType,
                               tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.meta_command_step), 1, -1,
                  "MetaCommandStepListType.MetaCommandStep")
    for e in v.meta_command_step:
        _w_MetaCommandStepType(w, e, 'MetaCommandStep')
    w.end(tag)


def _w_MetaCommandStepType(w: X.Writer, v: MetaCommandStepType,
                           tag: str) -> None:
    w.start(tag)
    w.attr('metaCommandRef', v.meta_command_ref)
    if v.argument_assignment_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_assignment_list,
                                      "ArgumentAssignmentList")
    w.end(tag)


def _w_MetaCommandType(w: X.Writer, v: MetaCommandType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.abstract is not None:
        w.attr('abstract', X.f_bool(v.abstract))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.base_meta_command is not None:
        _w_BaseMetaCommandType(w, v.base_meta_command, 'BaseMetaCommand')
    if v.system_name is not None:
        w.leaf('SystemName', v.system_name)
    if v.argument_list is not None:
        _w_ArgumentListType(w, v.argument_list, 'ArgumentList')
    if v.command_container is not None:
        _w_CommandContainerType(w, v.command_container, 'CommandContainer')
    if v.transmission_constraint_list is not None:
        _w_TransmissionConstraintListType(w, v.transmission_constraint_list,
                                          "TransmissionConstraintList")
    if v.default_significance is not None:
        _w_SignificanceType(w, v.default_significance, 'DefaultSignificance')
    if v.context_significance_list is not None:
        _w_ContextSignificanceListType(w, v.context_significance_list,
                                       "ContextSignificanceList")
    if v.interlock is not None:
        _w_InterlockType(w, v.interlock, 'Interlock')
    if v.verifier_set is not None:
        _w_VerifierSetType(w, v.verifier_set, 'VerifierSet')
    if v.parameter_to_set_list is not None:
        _w_ParameterToSetListType(w, v.parameter_to_set_list,
                                  "ParameterToSetList")
    if v.parameters_to_suspend_alarms_on_set is not None:
        _w_ParametersToSuspendAlarmsOnSetType(
            w, v.parameters_to_suspend_alarms_on_set,
            "ParametersToSuspendAlarmsOnSet")
    w.end(tag)


def _w_MultiRangeType(w: X.Writer, v: MultiRangeType, tag: str) -> None:
    w.start(tag)
    if v.min_inclusive is not None:
        w.attr('minInclusive', X.f_double(v.min_inclusive))
    if v.min_exclusive is not None:
        w.attr('minExclusive', X.f_double(v.min_exclusive))
    if v.max_inclusive is not None:
        w.attr('maxInclusive', X.f_double(v.max_inclusive))
    if v.max_exclusive is not None:
        w.attr('maxExclusive', X.f_double(v.max_exclusive))
    if v.range_form is not None:
        w.attr('rangeForm', v.range_form.value)
    if v.level is not None:
        w.attr('level', v.level.value)
    w.end(tag)


def _w_NameDescriptionType(w: X.Writer, v: NameDescriptionType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_NoteSetType(w: X.Writer, v: NoteSetType, tag: str) -> None:
    w.start(tag)
    for e in v.note:
        w.leaf('Note', e)
    w.end(tag)


def _w_NumberFormatType(w: X.Writer, v: NumberFormatType, tag: str) -> None:
    w.start(tag)
    if v.number_base is not None:
        w.attr('numberBase', v.number_base.value)
    if v.minimum_fraction_digits is not None:
        w.attr('minimumFractionDigits', X.f_int(v.minimum_fraction_digits))
    if v.maximum_fraction_digits is not None:
        w.attr('maximumFractionDigits', X.f_int(v.maximum_fraction_digits))
    if v.minimum_integer_digits is not None:
        w.attr('minimumIntegerDigits', X.f_int(v.minimum_integer_digits))
    if v.maximum_integer_digits is not None:
        w.attr('maximumIntegerDigits', X.f_int(v.maximum_integer_digits))
    if v.negative_suffix is not None:
        w.attr('negativeSuffix', v.negative_suffix)
    if v.positive_suffix is not None:
        w.attr('positiveSuffix', v.positive_suffix)
    if v.negative_prefix is not None:
        w.attr('negativePrefix', v.negative_prefix)
    if v.positive_prefix is not None:
        w.attr('positivePrefix', v.positive_prefix)
    if v.show_thousands_grouping is not None:
        w.attr('showThousandsGrouping', X.f_bool(v.show_thousands_grouping))
    if v.notation is not None:
        w.attr('notation', v.notation.value)
    w.end(tag)


def _w_NumericAlarmType(w: X.Writer, v: NumericAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_NumericAlarmChoice(w, v.choice)
    if v.static_alarm_ranges is not None:
        _w_AlarmRangesType(w, v.static_alarm_ranges, 'StaticAlarmRanges')
    if v.change_alarm_ranges is not None:
        _w_ChangeAlarmRangesType(w, v.change_alarm_ranges, 'ChangeAlarmRanges')
    if v.alarm_multi_ranges is not None:
        _w_AlarmMultiRangesType(w, v.alarm_multi_ranges, 'AlarmMultiRanges')
    w.end(tag)


def _w_NumericContextAlarmListType(w: X.Writer, v: NumericContextAlarmListType,
                                   tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_alarm), 1, -1,
                  "NumericContextAlarmListType.ContextAlarm")
    for e in v.context_alarm:
        _w_NumericContextAlarmType(w, e, 'ContextAlarm')
    w.end(tag)


def _w_NumericContextAlarmType(w: X.Writer, v: NumericContextAlarmType,
                               tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_NumericContextAlarmChoice(w, v.choice)
    if v.static_alarm_ranges is not None:
        _w_AlarmRangesType(w, v.static_alarm_ranges, 'StaticAlarmRanges')
    if v.change_alarm_ranges is not None:
        _w_ChangeAlarmRangesType(w, v.change_alarm_ranges, 'ChangeAlarmRanges')
    if v.alarm_multi_ranges is not None:
        _w_AlarmMultiRangesType(w, v.alarm_multi_ranges, 'AlarmMultiRanges')
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    w.end(tag)


def _w_ORedConditionsType(w: X.Writer, v: ORedConditionsType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 2, -1,
                  "ORedConditionsType.ORedConditionsChoice")
    for e in v.items:
        _w_ORedConditionsChoice(w, e)
    w.end(tag)


def _w_OnContainerUpdateTriggerType(
        w: X.Writer, v: OnContainerUpdateTriggerType, tag: str) -> None:
    w.start(tag)
    w.attr('containerRef', v.container_ref)
    w.end(tag)


def _w_OnParameterUpdateTriggerType(
        w: X.Writer, v: OnParameterUpdateTriggerType, tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    w.end(tag)


def _w_OnPeriodicRateTriggerType(w: X.Writer, v: OnPeriodicRateTriggerType,
                                 tag: str) -> None:
    w.start(tag)
    w.attr('fireRateInSeconds', X.f_double(v.fire_rate_in_seconds))
    w.end(tag)


def _w_OptionalNameDescriptionType(w: X.Writer, v: OptionalNameDescriptionType,
                                   tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_OutputParameterRefType(w: X.Writer, v: OutputParameterRefType,
                              tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    if v.output_name is not None:
        w.attr('outputName', v.output_name)
    w.end(tag)


def _w_OutputSetType(w: X.Writer, v: OutputSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.output_parameter_ref), 1, -1,
                  "OutputSetType.OutputParameterRef")
    for e in v.output_parameter_ref:
        _w_OutputParameterRefType(w, e, 'OutputParameterRef')
    w.end(tag)


def _w_PCMStreamType(w: X.Writer, v: PCMStreamType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.bit_rate_in_bps is not None:
        w.attr('bitRateInBPS', X.f_double(v.bit_rate_in_bps))
    if v.pcm_type is not None:
        w.attr('pcmType', v.pcm_type.value)
    if v.inverted is not None:
        w.attr('inverted', X.f_bool(v.inverted))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ParameterInstanceRefType(w: X.Writer, v: ParameterInstanceRefType,
                                tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    if v.instance is not None:
        w.attr('instance', X.f_int(v.instance))
    if v.use_calibrated_value is not None:
        w.attr('useCalibratedValue', X.f_bool(v.use_calibrated_value))
    w.end(tag)


def _w_ParameterPropertiesType(w: X.Writer, v: ParameterPropertiesType,
                               tag: str) -> None:
    w.start(tag)
    if v.data_source is not None:
        w.attr('dataSource', v.data_source.value)
    if v.read_only is not None:
        w.attr('readOnly', X.f_bool(v.read_only))
    if v.persistence is not None:
        w.attr('persistence', X.f_bool(v.persistence))
    if v.system_name is not None:
        w.leaf('SystemName', v.system_name)
    if v.validity_condition is not None:
        _w_MatchCriteriaType(w, v.validity_condition, 'ValidityCondition')
    if v.physical_address_set is not None:
        _w_PhysicalAddressSetType(w, v.physical_address_set,
                                  "PhysicalAddressSet")
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    w.end(tag)


def _w_ParameterRefEntryType(w: X.Writer, v: ParameterRefEntryType,
                             tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('parameterRef', v.parameter_ref)
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ParameterRefType(w: X.Writer, v: ParameterRefType, tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    w.end(tag)


def _w_ParameterSegmentRefEntryType(
        w: X.Writer, v: ParameterSegmentRefEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('parameterRef', v.parameter_ref)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ParameterSetType(w: X.Writer, v: ParameterSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1, 'ParameterSetType.ParameterSetChoice')
    for e in v.items:
        _w_ParameterSetChoice(w, e)
    w.end(tag)


def _w_ParameterToSetListType(w: X.Writer, v: ParameterToSetListType,
                              tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.parameter_to_set), 1, -1,
                  "ParameterToSetListType.ParameterToSet")
    for e in v.parameter_to_set:
        _w_ParameterToSetType(w, e, 'ParameterToSet')
    w.end(tag)


def _w_ParameterToSetType(w: X.Writer, v: ParameterToSetType, tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    if v.set_on_verification is not None:
        w.attr('setOnVerification', v.set_on_verification.value)
    _w_ParameterToSetChoice(w, v.choice)
    w.end(tag)


def _w_ParameterToSuspendAlarmsOnType(
        w: X.Writer, v: ParameterToSuspendAlarmsOnType, tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    w.attr('suspenseTime', v.suspense_time)
    if v.verifier_to_trigger_on is not None:
        w.attr('verifierToTriggerOn', v.verifier_to_trigger_on.value)
    w.end(tag)


def _w_ParameterType(w: X.Writer, v: ParameterType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    w.attr('parameterTypeRef', v.parameter_type_ref)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.parameter_properties is not None:
        _w_ParameterPropertiesType(w, v.parameter_properties,
                                   "ParameterProperties")
    w.end(tag)


def _w_ParameterTypeSetType(w: X.Writer, v: ParameterTypeSetType,
                            tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1,
                  "ParameterTypeSetType.ParameterTypeSetChoice")
    for e in v.items:
        _w_ParameterTypeSetChoice(w, e)
    w.end(tag)


def _w_ParameterValueChangeType(w: X.Writer, v: ParameterValueChangeType,
                                tag: str) -> None:
    w.start(tag)
    _w_ParameterRefType(w, v.parameter_ref, 'ParameterRef')
    _w_ChangeValueType(w, v.change, 'Change')
    w.end(tag)


def _w_ParametersToSuspendAlarmsOnSetType(
        w: X.Writer, v: ParametersToSuspendAlarmsOnSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(
        len(v.parameter_to_suspend_alarms_on), 1, -1,
        "ParametersToSuspendAlarmsOnSetType.ParameterToSuspendAlarmsOn")
    for e in v.parameter_to_suspend_alarms_on:
        _w_ParameterToSuspendAlarmsOnType(w, e, 'ParameterToSuspendAlarmsOn')
    w.end(tag)


def _w_ParityType(w: X.Writer, v: ParityType, tag: str) -> None:
    w.start(tag)
    w.attr('type', v.type.value)
    if v.bits_from_reference is not None:
        w.attr('bitsFromReference', X.f_int(v.bits_from_reference))
    if v.reference is not None:
        w.attr('reference', v.reference.value)
    if v.parameter_ref is not None:
        w.attr('parameterRef', v.parameter_ref)
    w.end(tag)


def _w_PercentCompleteType(w: X.Writer, v: PercentCompleteType,
                           tag: str) -> None:
    w.start(tag)
    _w_PercentCompleteChoice(w, v.choice)
    w.end(tag)


def _w_PhysicalAddressSetType(w: X.Writer, v: PhysicalAddressSetType,
                              tag: str) -> None:
    w.start(tag)
    for e in v.physical_address:
        _w_PhysicalAddressType(w, e, 'PhysicalAddress')
    w.end(tag)


def _w_PhysicalAddressType(w: X.Writer, v: PhysicalAddressType,
                           tag: str) -> None:
    w.start(tag)
    if v.source_name is not None:
        w.attr('sourceName', v.source_name)
    if v.source_address is not None:
        w.attr('sourceAddress', v.source_address)
    if v.sub_address is not None:
        _w_PhysicalAddressType(w, v.sub_address, 'SubAddress')
    w.end(tag)


def _w_PolynomialCalibratorType(w: X.Writer, v: PolynomialCalibratorType,
                                tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.term), 1, -1, 'PolynomialCalibratorType.Term')
    for e in v.term:
        _w_TermType(w, e, 'Term')
    w.end(tag)


def _w_QueuedVerifierType(w: X.Writer, v: QueuedVerifierType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_QueuedVerifierChoice1(w, v.choice1)
    _w_QueuedVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_RateInStreamSetType(w: X.Writer, v: RateInStreamSetType,
                           tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.rate_in_stream), 1, -1,
                  "RateInStreamSetType.RateInStream")
    for e in v.rate_in_stream:
        _w_RateInStreamWithStreamNameType(w, e, 'RateInStream')
    w.end(tag)


def _w_RateInStreamType(w: X.Writer, v: RateInStreamType, tag: str) -> None:
    w.start(tag)
    if v.basis is not None:
        w.attr('basis', v.basis.value)
    if v.minimum_value is not None:
        w.attr('minimumValue', X.f_double(v.minimum_value))
    if v.maximum_value is not None:
        w.attr('maximumValue', X.f_double(v.maximum_value))
    w.end(tag)


def _w_RateInStreamWithStreamNameType(
        w: X.Writer, v: RateInStreamWithStreamNameType, tag: str) -> None:
    w.start(tag)
    if v.basis is not None:
        w.attr('basis', v.basis.value)
    if v.minimum_value is not None:
        w.attr('minimumValue', X.f_double(v.minimum_value))
    if v.maximum_value is not None:
        w.attr('maximumValue', X.f_double(v.maximum_value))
    w.attr('streamRef', v.stream_ref)
    w.end(tag)


def _w_ReceivedVerifierType(w: X.Writer, v: ReceivedVerifierType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_ReceivedVerifierChoice1(w, v.choice1)
    _w_ReceivedVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_ReferenceTimeType(w: X.Writer, v: ReferenceTimeType, tag: str) -> None:
    w.start(tag)
    _w_ReferenceTimeChoice(w, v.choice)
    w.end(tag)


def _w_RelativeTimeArgumentType(w: X.Writer, v: RelativeTimeArgumentType,
                                tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_RelativeTimeDataType(w: X.Writer, v: RelativeTimeDataType,
                            tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    w.end(tag)


def _w_RelativeTimeParameterType(w: X.Writer, v: RelativeTimeParameterType,
                                 tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.encoding is not None:
        _w_EncodingType(w, v.encoding, 'Encoding')
    if v.reference_time is not None:
        _w_ReferenceTimeType(w, v.reference_time, 'ReferenceTime')
    if v.default_alarm is not None:
        _w_TimeAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_TimeContextAlarmListType(w, v.context_alarm_list, 'ContextAlarmList')
    w.end(tag)


def _w_RepeatType(w: X.Writer, v: RepeatType, tag: str) -> None:
    w.start(tag)
    _w_IntegerValueType(w, v.count, 'Count')
    if v.offset is not None:
        _w_IntegerValueType(w, v.offset, 'Offset')
    w.end(tag)


def _w_RestrictionCriteriaType(w: X.Writer, v: RestrictionCriteriaType,
                               tag: str) -> None:
    w.start(tag)
    _w_RestrictionCriteriaChoice(w, v.choice)
    if v.next_container is not None:
        _w_ContainerRefType(w, v.next_container, 'NextContainer')
    w.end(tag)


def _w_SentFromRangeVerifierType(w: X.Writer, v: SentFromRangeVerifierType,
                                 tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_SentFromRangeVerifierChoice1(w, v.choice1)
    _w_SentFromRangeVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_SequenceContainerType(w: X.Writer, v: SequenceContainerType,
                             tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.abstract is not None:
        w.attr('abstract', X.f_bool(v.abstract))
    if v.idle_pattern is not None:
        w.attr('idlePattern', v.idle_pattern)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.default_rate_in_stream is not None:
        _w_RateInStreamType(w, v.default_rate_in_stream, 'DefaultRateInStream')
    if v.rate_in_stream_set is not None:
        _w_RateInStreamSetType(w, v.rate_in_stream_set, 'RateInStreamSet')
    if v.binary_encoding is not None:
        _w_ContainerBinaryDataEncodingType(w, v.binary_encoding,
                                           "BinaryEncoding")
    _w_EntryListType(w, v.entry_list, 'EntryList')
    if v.base_container is not None:
        _w_BaseContainerType(w, v.base_container, 'BaseContainer')
    w.end(tag)


def _w_SequenceEntryType(w: X.Writer, v: SequenceEntryType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_ServiceRefType(w: X.Writer, v: ServiceRefType, tag: str) -> None:
    w.start(tag)
    w.attr('serviceRef', v.service_ref)
    w.end(tag)


def _w_ServiceSetType(w: X.Writer, v: ServiceSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.service), 1, -1, 'ServiceSetType.Service')
    for e in v.service:
        _w_ServiceType(w, e, 'Service')
    w.end(tag)


def _w_ServiceType(w: X.Writer, v: ServiceType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_ServiceChoice(w, v.choice)
    w.end(tag)


def _w_SignificanceType(w: X.Writer, v: SignificanceType, tag: str) -> None:
    w.start(tag)
    if v.space_system_at_risk is not None:
        w.attr('spaceSystemAtRisk', v.space_system_at_risk)
    if v.reason_for_warning is not None:
        w.attr('reasonForWarning', v.reason_for_warning)
    if v.consequence_level is not None:
        w.attr('consequenceLevel', v.consequence_level.value)
    w.end(tag)


def _w_SimpleAlgorithmType(w: X.Writer, v: SimpleAlgorithmType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.algorithm_text is not None:
        _w_AlgorithmTextType(w, v.algorithm_text, 'AlgorithmText')
    if v.external_algorithm_set is not None:
        _w_ExternalAlgorithmSetType(w, v.external_algorithm_set,
                                    "ExternalAlgorithmSet")
    w.end(tag)


def _w_SizeInBitsFixedType(w: X.Writer, v: SizeInBitsFixedType,
                           tag: str) -> None:
    w.start(tag)
    w.leaf('FixedValue', X.f_int(v.fixed_value))
    w.end(tag)


def _w_SizeInBitsType(w: X.Writer, v: SizeInBitsType, tag: str) -> None:
    w.start(tag)
    _w_SizeInBitsFixedType(w, v.fixed, 'Fixed')
    if v.termination_char is not None:
        w.leaf('TerminationChar', v.termination_char)
    if v.leading_size is not None:
        _w_LeadingSizeType(w, v.leading_size, 'LeadingSize')
    w.end(tag)


def _w_SpaceSystemType(w: X.Writer, v: SpaceSystemType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.system_type is not None:
        w.attr('systemType', v.system_type.value)
    if v.asset_type is not None:
        w.attr('assetType', v.asset_type)
    if v.operational_status is not None:
        w.attr('operationalStatus', v.operational_status)
    if v.xml_base is not None:
        w.attr('xml:base', v.xml_base)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.header is not None:
        _w_HeaderType(w, v.header, 'Header')
    if v.telemetry_meta_data is not None:
        _w_TelemetryMetaDataType(w, v.telemetry_meta_data, 'TelemetryMetaData')
    if v.command_meta_data is not None:
        _w_CommandMetaDataType(w, v.command_meta_data, 'CommandMetaData')
    if v.service_set is not None:
        _w_ServiceSetType(w, v.service_set, 'ServiceSet')
    for e in v.space_system:
        _w_SpaceSystemType(w, e, 'SpaceSystem')
    w.end(tag)


def _w_SplineCalibratorType(w: X.Writer, v: SplineCalibratorType,
                            tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    if v.extrapolate is not None:
        w.attr('extrapolate', X.f_bool(v.extrapolate))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.spline_point), 2, -1,
                  "SplineCalibratorType.SplinePoint")
    for e in v.spline_point:
        _w_SplinePointType(w, e, 'SplinePoint')
    w.end(tag)


def _w_SplinePointType(w: X.Writer, v: SplinePointType, tag: str) -> None:
    w.start(tag)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('raw', X.f_double(v.raw))
    w.attr('calibrated', X.f_double(v.calibrated))
    w.end(tag)


def _w_StreamRefType(w: X.Writer, v: StreamRefType, tag: str) -> None:
    w.start(tag)
    w.attr('streamRef', v.stream_ref)
    w.end(tag)


def _w_StreamSegmentEntryType(w: X.Writer, v: StreamSegmentEntryType,
                              tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('streamRef', v.stream_ref)
    if v.order is not None:
        w.attr('order', X.f_int(v.order))
    w.attr('sizeInBits', X.f_int(v.size_in_bits))
    if v.location_in_container_in_bits is not None:
        _w_LocationInContainerInBitsType(w, v.location_in_container_in_bits,
                                         "LocationInContainerInBits")
    if v.repeat_entry is not None:
        _w_RepeatType(w, v.repeat_entry, 'RepeatEntry')
    if v.include_condition is not None:
        _w_MatchCriteriaType(w, v.include_condition, 'IncludeCondition')
    if v.time_association is not None:
        _w_TimeAssociationType(w, v.time_association, 'TimeAssociation')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    w.end(tag)


def _w_StreamSetType(w: X.Writer, v: StreamSetType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.items), 1, -1, 'StreamSetType.StreamSetChoice')
    for e in v.items:
        _w_StreamSetChoice(w, e)
    w.end(tag)


def _w_StringAlarmLevelType(w: X.Writer, v: StringAlarmLevelType,
                            tag: str) -> None:
    w.start(tag)
    w.attr('alarmLevel', v.alarm_level.value)
    w.attr('matchPattern', v.match_pattern)
    w.end(tag)


def _w_StringAlarmListType(w: X.Writer, v: StringAlarmListType,
                           tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.string_alarm), 1, -1, 'StringAlarmListType.StringAlarm')
    for e in v.string_alarm:
        _w_StringAlarmLevelType(w, e, 'StringAlarm')
    w.end(tag)


def _w_StringAlarmType(w: X.Writer, v: StringAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.default_alarm_level is not None:
        w.attr('defaultAlarmLevel', v.default_alarm_level.value)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_StringAlarmChoice(w, v.choice)
    if v.string_alarm_list is not None:
        _w_StringAlarmListType(w, v.string_alarm_list, 'StringAlarmList')
    w.end(tag)


def _w_StringArgumentType(w: X.Writer, v: StringArgumentType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.restriction_pattern is not None:
        w.attr('restrictionPattern', v.restriction_pattern)
    if v.character_width is not None:
        w.attr('characterWidth', v.character_width.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_StringArgumentChoice(w, v.choice)
    if v.size_range_in_characters is not None:
        _w_IntegerRangeType(w, v.size_range_in_characters,
                            "SizeRangeInCharacters")
    w.end(tag)


def _w_StringContextAlarmListType(w: X.Writer, v: StringContextAlarmListType,
                                  tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_alarm), 1, -1,
                  "StringContextAlarmListType.ContextAlarm")
    for e in v.context_alarm:
        _w_StringContextAlarmType(w, e, 'ContextAlarm')
    w.end(tag)


def _w_StringContextAlarmType(w: X.Writer, v: StringContextAlarmType,
                              tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.default_alarm_level is not None:
        w.attr('defaultAlarmLevel', v.default_alarm_level.value)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_StringContextAlarmChoice(w, v.choice)
    if v.string_alarm_list is not None:
        _w_StringAlarmListType(w, v.string_alarm_list, 'StringAlarmList')
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    w.end(tag)


def _w_StringDataEncodingType(w: X.Writer, v: StringDataEncodingType,
                              tag: str) -> None:
    w.start(tag)
    if v.bit_order is not None:
        w.attr('bitOrder', v.bit_order.value)
    if v.byte_order is not None:
        w.attr('byteOrder', v.byte_order)
    if v.encoding is not None:
        w.attr('encoding', v.encoding.value)
    if v.error_detect_correct is not None:
        _w_ErrorDetectCorrectType(w, v.error_detect_correct,
                                  "ErrorDetectCorrect")
    _w_StringDataEncodingChoice(w, v.choice)
    w.end(tag)


def _w_StringDataType(w: X.Writer, v: StringDataType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.restriction_pattern is not None:
        w.attr('restrictionPattern', v.restriction_pattern)
    if v.character_width is not None:
        w.attr('characterWidth', v.character_width.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_StringDataChoice(w, v.choice)
    if v.size_range_in_characters is not None:
        _w_IntegerRangeType(w, v.size_range_in_characters,
                            "SizeRangeInCharacters")
    w.end(tag)


def _w_StringParameterType(w: X.Writer, v: StringParameterType,
                           tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.base_type is not None:
        w.attr('baseType', v.base_type)
    if v.initial_value is not None:
        w.attr('initialValue', v.initial_value)
    if v.restriction_pattern is not None:
        w.attr('restrictionPattern', v.restriction_pattern)
    if v.character_width is not None:
        w.attr('characterWidth', v.character_width.value)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.unit_set is not None:
        _w_UnitSetType(w, v.unit_set, 'UnitSet')
    if v.choice is not None:
        _w_StringParameterChoice(w, v.choice)
    if v.size_range_in_characters is not None:
        _w_IntegerRangeType(w, v.size_range_in_characters,
                            "SizeRangeInCharacters")
    if v.default_alarm is not None:
        _w_StringAlarmType(w, v.default_alarm, 'DefaultAlarm')
    if v.context_alarm_list is not None:
        _w_StringContextAlarmListType(w, v.context_alarm_list,
                                      "ContextAlarmList")
    w.end(tag)


def _w_SyncPatternType(w: X.Writer, v: SyncPatternType, tag: str) -> None:
    w.start(tag)
    w.attr('pattern', v.pattern)
    if v.bit_location_from_start_of_container is not None:
        w.attr("bitLocationFromStartOfContainer",
               X.f_int(v.bit_location_from_start_of_container))
    if v.mask is not None:
        w.attr('mask', v.mask)
    if v.mask_length_in_bits is not None:
        w.attr('maskLengthInBits', X.f_int(v.mask_length_in_bits))
    w.attr('patternLengthInBits', X.f_int(v.pattern_length_in_bits))
    w.end(tag)


def _w_SyncStrategyType(w: X.Writer, v: SyncStrategyType, tag: str) -> None:
    w.start(tag)
    if v.verify_to_lock_good_frames is not None:
        w.attr('verifyToLockGoodFrames', X.f_int(v.verify_to_lock_good_frames))
    if v.check_to_lock_good_frames is not None:
        w.attr('checkToLockGoodFrames', X.f_int(v.check_to_lock_good_frames))
    if v.max_bit_errors_in_sync_pattern is not None:
        w.attr("maxBitErrorsInSyncPattern",
               X.f_int(v.max_bit_errors_in_sync_pattern))
    if v.auto_invert is not None:
        _w_AutoInvertType(w, v.auto_invert, 'AutoInvert')
    w.end(tag)


def _w_TelemetryMetaDataType(w: X.Writer, v: TelemetryMetaDataType,
                             tag: str) -> None:
    w.start(tag)
    if v.parameter_type_set is not None:
        _w_ParameterTypeSetType(w, v.parameter_type_set, 'ParameterTypeSet')
    if v.parameter_set is not None:
        _w_ParameterSetType(w, v.parameter_set, 'ParameterSet')
    if v.container_set is not None:
        _w_ContainerSetType(w, v.container_set, 'ContainerSet')
    if v.message_set is not None:
        _w_MessageSetType(w, v.message_set, 'MessageSet')
    if v.stream_set is not None:
        _w_StreamSetType(w, v.stream_set, 'StreamSet')
    if v.algorithm_set is not None:
        _w_AlgorithmSetType(w, v.algorithm_set, 'AlgorithmSet')
    w.end(tag)


def _w_TermType(w: X.Writer, v: TermType, tag: str) -> None:
    w.start(tag)
    w.attr('coefficient', X.f_double(v.coefficient))
    w.attr('exponent', X.f_int(v.exponent))
    w.end(tag)


def _w_TimeAlarmRangesType(w: X.Writer, v: TimeAlarmRangesType,
                           tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.range_form is not None:
        w.attr('rangeForm', v.range_form.value)
    if v.time_units is not None:
        w.attr('timeUnits', v.time_units.value)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.watch_range is not None:
        _w_FloatRangeType(w, v.watch_range, 'WatchRange')
    if v.warning_range is not None:
        _w_FloatRangeType(w, v.warning_range, 'WarningRange')
    if v.distress_range is not None:
        _w_FloatRangeType(w, v.distress_range, 'DistressRange')
    if v.critical_range is not None:
        _w_FloatRangeType(w, v.critical_range, 'CriticalRange')
    if v.severe_range is not None:
        _w_FloatRangeType(w, v.severe_range, 'SevereRange')
    w.end(tag)


def _w_TimeAlarmType(w: X.Writer, v: TimeAlarmType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_TimeAlarmChoice(w, v.choice)
    if v.static_alarm_ranges is not None:
        _w_TimeAlarmRangesType(w, v.static_alarm_ranges, 'StaticAlarmRanges')
    if v.change_per_second_alarm_ranges is not None:
        _w_TimeAlarmRangesType(w, v.change_per_second_alarm_ranges,
                               "ChangePerSecondAlarmRanges")
    w.end(tag)


def _w_TimeAssociationType(w: X.Writer, v: TimeAssociationType,
                           tag: str) -> None:
    w.start(tag)
    w.attr('parameterRef', v.parameter_ref)
    if v.instance is not None:
        w.attr('instance', X.f_int(v.instance))
    if v.use_calibrated_value is not None:
        w.attr('useCalibratedValue', X.f_bool(v.use_calibrated_value))
    if v.interpolate_time is not None:
        w.attr('interpolateTime', X.f_bool(v.interpolate_time))
    if v.offset is not None:
        w.attr('offset', X.f_double(v.offset))
    if v.unit is not None:
        w.attr('unit', v.unit.value)
    w.end(tag)


def _w_TimeContextAlarmListType(w: X.Writer, v: TimeContextAlarmListType,
                                tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.context_alarm), 1, -1,
                  "TimeContextAlarmListType.ContextAlarm")
    for e in v.context_alarm:
        _w_TimeContextAlarmType(w, e, 'ContextAlarm')
    w.end(tag)


def _w_TimeContextAlarmType(w: X.Writer, v: TimeContextAlarmType,
                            tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.min_violations is not None:
        w.attr('minViolations', X.f_int(v.min_violations))
    if v.min_conformance is not None:
        w.attr('minConformance', X.f_int(v.min_conformance))
    if v.disabled is not None:
        w.attr('disabled', X.f_bool(v.disabled))
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    if v.choice is not None:
        _w_TimeContextAlarmChoice(w, v.choice)
    if v.static_alarm_ranges is not None:
        _w_TimeAlarmRangesType(w, v.static_alarm_ranges, 'StaticAlarmRanges')
    if v.change_per_second_alarm_ranges is not None:
        _w_TimeAlarmRangesType(w, v.change_per_second_alarm_ranges,
                               "ChangePerSecondAlarmRanges")
    _w_ContextMatchType(w, v.context_match, 'ContextMatch')
    w.end(tag)


def _w_ToStringType(w: X.Writer, v: ToStringType, tag: str) -> None:
    w.start(tag)
    _w_NumberFormatType(w, v.number_format, 'NumberFormat')
    w.end(tag)


def _w_TransferredToRangeVerifierType(
        w: X.Writer, v: TransferredToRangeVerifierType, tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    if v.name is not None:
        w.attr('name', v.name)
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_TransferredToRangeVerifierChoice1(w, v.choice1)
    _w_TransferredToRangeVerifierChoice2(w, v.choice2)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_TransmissionConstraintListType(
        w: X.Writer, v: TransmissionConstraintListType, tag: str) -> None:
    w.start(tag)
    X.check_count(len(v.transmission_constraint), 1, -1,
                  "TransmissionConstraintListType.TransmissionConstraint")
    for e in v.transmission_constraint:
        _w_TransmissionConstraintType(w, e, 'TransmissionConstraint')
    w.end(tag)


def _w_TransmissionConstraintType(w: X.Writer, v: TransmissionConstraintType,
                                  tag: str) -> None:
    w.start(tag)
    if v.time_out is not None:
        w.attr('timeOut', v.time_out)
    if v.suspendable is not None:
        w.attr('suspendable', X.f_bool(v.suspendable))
    _w_TransmissionConstraintChoice(w, v.choice)
    if v.argument_restriction_list is not None:
        _w_ArgumentAssignmentListType(w, v.argument_restriction_list,
                                      "ArgumentRestrictionList")
    w.end(tag)


def _w_TriggerSetType(w: X.Writer, v: TriggerSetType, tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.trigger_rate is not None:
        w.attr('triggerRate', X.f_int(v.trigger_rate))
    X.check_count(len(v.items), 1, -1, 'TriggerSetType.TriggerSetChoice')
    for e in v.items:
        _w_TriggerSetChoice(w, e)
    w.end(tag)


def _w_TriggeredMathOperationType(w: X.Writer, v: TriggeredMathOperationType,
                                  tag: str) -> None:
    w.start(tag)
    if v.name is not None:
        w.attr('name', v.name)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('outputParameterRef', v.output_parameter_ref)
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    X.check_count(len(v.items), 1, -1,
                  "TriggeredMathOperationType.TriggeredMathOperationChoice")
    for e in v.items:
        _w_TriggeredMathOperationChoice(w, e)
    _w_TriggerSetType(w, v.trigger_set, 'TriggerSet')
    w.end(tag)


def _w_UnitSetType(w: X.Writer, v: UnitSetType, tag: str) -> None:
    w.start(tag)
    for e in v.unit:
        _w_UnitType(w, e, 'Unit')
    w.end(tag)


def _w_UnitType(w: X.Writer, v: UnitType, tag: str) -> None:
    w.start(tag)
    if v.power is not None:
        w.attr('power', X.f_double(v.power))
    if v.factor is not None:
        w.attr('factor', v.factor)
    if v.description is not None:
        w.attr('description', v.description)
    if v.form is not None:
        w.attr('form', v.form.value)
    w.text(v.value)
    w.end(tag)


def _w_ValidFloatRangeSetType(w: X.Writer, v: ValidFloatRangeSetType,
                              tag: str) -> None:
    w.start(tag)
    if v.valid_range_applies_to_calibrated is not None:
        w.attr("validRangeAppliesToCalibrated",
               X.f_bool(v.valid_range_applies_to_calibrated))
    X.check_count(len(v.valid_range), 1, -1,
                  "ValidFloatRangeSetType.ValidRange")
    for e in v.valid_range:
        _w_FloatRangeType(w, e, 'ValidRange')
    w.end(tag)


def _w_ValidIntegerRangeSetType(w: X.Writer, v: ValidIntegerRangeSetType,
                                tag: str) -> None:
    w.start(tag)
    if v.valid_range_applies_to_calibrated is not None:
        w.attr("validRangeAppliesToCalibrated",
               X.f_bool(v.valid_range_applies_to_calibrated))
    X.check_count(len(v.valid_range), 1, -1,
                  "ValidIntegerRangeSetType.ValidRange")
    for e in v.valid_range:
        _w_IntegerRangeType(w, e, 'ValidRange')
    w.end(tag)


def _w_ValueEnumerationType(w: X.Writer, v: ValueEnumerationType,
                            tag: str) -> None:
    w.start(tag)
    w.attr('value', X.f_int(v.value))
    if v.max_value is not None:
        w.attr('maxValue', X.f_int(v.max_value))
    w.attr('label', v.label)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.end(tag)


def _w_VariableFrameStreamType(w: X.Writer, v: VariableFrameStreamType,
                               tag: str) -> None:
    w.start(tag)
    if v.short_description is not None:
        w.attr('shortDescription', v.short_description)
    w.attr('name', v.name)
    if v.bit_rate_in_bps is not None:
        w.attr('bitRateInBPS', X.f_double(v.bit_rate_in_bps))
    if v.pcm_type is not None:
        w.attr('pcmType', v.pcm_type.value)
    if v.inverted is not None:
        w.attr('inverted', X.f_bool(v.inverted))
    if v.long_description is not None:
        w.leaf('LongDescription', v.long_description)
    if v.alias_set is not None:
        _w_AliasSetType(w, v.alias_set, 'AliasSet')
    if v.ancillary_data_set is not None:
        _w_AncillaryDataSetType(w, v.ancillary_data_set, 'AncillaryDataSet')
    _w_VariableFrameStreamChoice(w, v.choice)
    if v.stream_ref is not None:
        _w_StreamRefType(w, v.stream_ref, 'StreamRef')
    _w_VariableFrameSyncStrategyType(w, v.sync_strategy, 'SyncStrategy')
    w.end(tag)


def _w_VariableFrameSyncStrategyType(
        w: X.Writer, v: VariableFrameSyncStrategyType, tag: str) -> None:
    w.start(tag)
    if v.verify_to_lock_good_frames is not None:
        w.attr('verifyToLockGoodFrames', X.f_int(v.verify_to_lock_good_frames))
    if v.check_to_lock_good_frames is not None:
        w.attr('checkToLockGoodFrames', X.f_int(v.check_to_lock_good_frames))
    if v.max_bit_errors_in_sync_pattern is not None:
        w.attr("maxBitErrorsInSyncPattern",
               X.f_int(v.max_bit_errors_in_sync_pattern))
    if v.auto_invert is not None:
        _w_AutoInvertType(w, v.auto_invert, 'AutoInvert')
    _w_FlagType(w, v.flag, 'Flag')
    w.end(tag)


def _w_VariableStringType(w: X.Writer, v: VariableStringType, tag: str) -> None:
    w.start(tag)
    w.attr('maxSizeInBits', X.f_int(v.max_size_in_bits))
    if v.choice1 is not None:
        _w_VariableStringChoice1(w, v.choice1)
    _w_VariableStringChoice2(w, v.choice2)
    w.end(tag)


def _w_VerifierSetType(w: X.Writer, v: VerifierSetType, tag: str) -> None:
    w.start(tag)
    if v.transferred_to_range_verifier is not None:
        _w_TransferredToRangeVerifierType(w, v.transferred_to_range_verifier,
                                          "TransferredToRangeVerifier")
    if v.sent_from_range_verifier is not None:
        _w_SentFromRangeVerifierType(w, v.sent_from_range_verifier,
                                     "SentFromRangeVerifier")
    if v.received_verifier is not None:
        _w_ReceivedVerifierType(w, v.received_verifier, 'ReceivedVerifier')
    if v.accepted_verifier is not None:
        _w_AcceptedVerifierType(w, v.accepted_verifier, 'AcceptedVerifier')
    if v.queued_verifier is not None:
        _w_QueuedVerifierType(w, v.queued_verifier, 'QueuedVerifier')
    for e in v.execution_verifier:
        _w_ExecutionVerifierType(w, e, 'ExecutionVerifier')
    for e in v.complete_verifier:
        _w_CompleteVerifierType(w, e, 'CompleteVerifier')
    if v.failed_verifier is not None:
        _w_FailedVerifierType(w, v.failed_verifier, 'FailedVerifier')
    w.end(tag)


def _w_XORType(w: X.Writer, v: XORType, tag: str) -> None:
    w.start(tag)
    if v.bits_from_reference is not None:
        w.attr('bitsFromReference', X.f_int(v.bits_from_reference))
    if v.reference is not None:
        w.attr('reference', v.reference.value)
    if v.parameter_ref is not None:
        w.attr('parameterRef', v.parameter_ref)
    w.end(tag)


_WC_ANDedConditionsChoice = {"Condition": _w_ComparisonCheckType,
                             "ORedConditions": _w_ORedConditionsType}


def _w_ANDedConditionsChoice(w: X.Writer, v: ANDedConditionsChoice) -> None:
    fn = _WC_ANDedConditionsChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ANDedConditionsChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_AcceptedVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_AcceptedVerifierChoice1(w: X.Writer, v: AcceptedVerifierChoice1) -> None:
    fn = _WC_AcceptedVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("AcceptedVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_AcceptedVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_AcceptedVerifierChoice2(w: X.Writer, v: AcceptedVerifierChoice2) -> None:
    fn = _WC_AcceptedVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("AcceptedVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_AlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                   "CustomAlarm": _w_CustomAlarmType}


def _w_AlarmChoice(w: X.Writer, v: AlarmChoice) -> None:
    fn = _WC_AlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"AlarmChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_AlgorithmSetChoice = {"CustomAlgorithm": _w_InputOutputTriggerAlgorithmType,
                          "MathAlgorithm": _w_MathAlgorithmType}


def _w_AlgorithmSetChoice(w: X.Writer, v: AlgorithmSetChoice) -> None:
    fn = _WC_AlgorithmSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"AlgorithmSetChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentANDedConditionsChoice = {
    "Condition": _w_ArgumentComparisonCheckType,
    "ORedConditions": _w_ArgumentORedConditionsType}


def _w_ArgumentANDedConditionsChoice(w: X.Writer,
                                     v: ArgumentANDedConditionsChoice) -> None:
    fn = _WC_ArgumentANDedConditionsChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentANDedConditionsChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentBaseDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentBaseDataChoice(w: X.Writer, v: ArgumentBaseDataChoice) -> None:
    fn = _WC_ArgumentBaseDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentBaseDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentBinaryDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentBinaryDataChoice(w: X.Writer,
                                v: ArgumentBinaryDataChoice) -> None:
    fn = _WC_ArgumentBinaryDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentBinaryDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentBooleanDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentBooleanDataChoice(w: X.Writer,
                                 v: ArgumentBooleanDataChoice) -> None:
    fn = _WC_ArgumentBooleanDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentBooleanDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentBooleanExpressionChoice = {
    "Condition": _w_ArgumentComparisonCheckType,
    "ANDedConditions": _w_ArgumentANDedConditionsType,
    "ORedConditions": _w_ArgumentORedConditionsType}


def _w_ArgumentBooleanExpressionChoice(
        w: X.Writer, v: ArgumentBooleanExpressionChoice) -> None:
    fn = _WC_ArgumentBooleanExpressionChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentBooleanExpressionChoice: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentComparisonCheckChoice1 = {
    "ParameterInstanceRef": _w_ParameterInstanceRefType,
    "ArgumentInstanceRef": _w_ArgumentInstanceRefType}


def _w_ArgumentComparisonCheckChoice1(
        w: X.Writer, v: ArgumentComparisonCheckChoice1) -> None:
    fn = _WC_ArgumentComparisonCheckChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentComparisonCheckChoice1: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentComparisonCheckChoice2 = {
    "ParameterInstanceRef": _w_ParameterInstanceRefType,
    "ArgumentInstanceRef": _w_ArgumentInstanceRefType,
    "Value": lambda w, x, t: w.leaf(t, x)}


def _w_ArgumentComparisonCheckChoice2(
        w: X.Writer, v: ArgumentComparisonCheckChoice2) -> None:
    fn = _WC_ArgumentComparisonCheckChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentComparisonCheckChoice2: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentComparisonChoice = {
    "ParameterInstanceRef": _w_ParameterInstanceRefType,
    "ArgumentInstanceRef": _w_ArgumentInstanceRefType}


def _w_ArgumentComparisonChoice(w: X.Writer,
                                v: ArgumentComparisonChoice) -> None:
    fn = _WC_ArgumentComparisonChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentComparisonChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentDiscreteLookupChoice = {
    "Comparison": _w_ArgumentComparisonType,
    "ComparisonList": _w_ArgumentComparisonListType,
    "BooleanExpression": _w_ArgumentBooleanExpressionType,
    "CustomAlgorithm": _w_ArgumentInputAlgorithmType}


def _w_ArgumentDiscreteLookupChoice(w: X.Writer,
                                    v: ArgumentDiscreteLookupChoice) -> None:
    fn = _WC_ArgumentDiscreteLookupChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentDiscreteLookupChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentDynamicValueChoice = {
    "ArgumentInstanceRef": _w_ArgumentInstanceRefType,
    "ParameterInstanceRef": _w_ParameterInstanceRefType}


def _w_ArgumentDynamicValueChoice(w: X.Writer,
                                  v: ArgumentDynamicValueChoice) -> None:
    fn = _WC_ArgumentDynamicValueChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentDynamicValueChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentEnumeratedDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentEnumeratedDataChoice(w: X.Writer,
                                    v: ArgumentEnumeratedDataChoice) -> None:
    fn = _WC_ArgumentEnumeratedDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentEnumeratedDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentFloatDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentFloatDataChoice(w: X.Writer, v: ArgumentFloatDataChoice) -> None:
    fn = _WC_ArgumentFloatDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentFloatDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentInputSetChoice = {
    "InputParameterInstanceRef": _w_InputParameterInstanceRefType,
    "InputArgumentInstanceRef": _w_ArgumentInstanceRefType,
    "Constant": _w_ConstantType}


def _w_ArgumentInputSetChoice(w: X.Writer, v: ArgumentInputSetChoice) -> None:
    fn = _WC_ArgumentInputSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentInputSetChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentIntegerDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentIntegerDataChoice(w: X.Writer,
                                 v: ArgumentIntegerDataChoice) -> None:
    fn = _WC_ArgumentIntegerDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentIntegerDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentIntegerValueChoice = {
    "FixedValue": lambda w, x, t: w.leaf(t, X.f_int(x)),
    "DynamicValue": _w_ArgumentDynamicValueType,
    "DiscreteLookupList": _w_ArgumentDiscreteLookupListType}


def _w_ArgumentIntegerValueChoice(w: X.Writer,
                                  v: ArgumentIntegerValueChoice) -> None:
    fn = _WC_ArgumentIntegerValueChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentIntegerValueChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentLocationInContainerInBitsChoice = {
    "FixedValue": lambda w, x, t: w.leaf(t, X.f_int(x)),
    "DynamicValue": _w_ArgumentDynamicValueType,
    "DiscreteLookupList": _w_ArgumentDiscreteLookupListType}


def _w_ArgumentLocationInContainerInBitsChoice(
        w: X.Writer, v: ArgumentLocationInContainerInBitsChoice) -> None:
    fn = _WC_ArgumentLocationInContainerInBitsChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentLocationInContainerInBitsChoice: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentMatchCriteriaChoice = {
    "Comparison": _w_ArgumentComparisonType,
    "ComparisonList": _w_ArgumentComparisonListType,
    "BooleanExpression": _w_ArgumentBooleanExpressionType,
    "CustomAlgorithm": _w_ArgumentInputAlgorithmType}


def _w_ArgumentMatchCriteriaChoice(w: X.Writer,
                                   v: ArgumentMatchCriteriaChoice) -> None:
    fn = _WC_ArgumentMatchCriteriaChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentMatchCriteriaChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentMathOperationChoice = {
    "ValueOperand": lambda w, x, t: w.leaf(t, x),
    "ThisParameterOperand": lambda w, x, t: w.leaf(t, x),
    "Operator": lambda w, x, t: w.leaf(t, x.value),
    "ParameterInstanceRefOperand": _w_ParameterInstanceRefType,
    "ArgumentInstanceRefOperand": _w_ArgumentInstanceRefType}


def _w_ArgumentMathOperationChoice(w: X.Writer,
                                   v: ArgumentMathOperationChoice) -> None:
    fn = _WC_ArgumentMathOperationChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentMathOperationChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentORedConditionsChoice = {
    "Condition": _w_ArgumentComparisonCheckType,
    "ANDedConditions": _w_ArgumentANDedConditionsType}


def _w_ArgumentORedConditionsChoice(w: X.Writer,
                                    v: ArgumentORedConditionsChoice) -> None:
    fn = _WC_ArgumentORedConditionsChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentORedConditionsChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentStringDataChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_ArgumentStringDataChoice(w: X.Writer,
                                v: ArgumentStringDataChoice) -> None:
    fn = _WC_ArgumentStringDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentStringDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentStringDataEncodingChoice = {
    "SizeInBits": _w_SizeInBitsType, "Variable": _w_ArgumentVariableStringType}


def _w_ArgumentStringDataEncodingChoice(
        w: X.Writer, v: ArgumentStringDataEncodingChoice) -> None:
    fn = _WC_ArgumentStringDataEncodingChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentStringDataEncodingChoice: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentTypeSetChoice = {
    "StringArgumentType": _w_StringArgumentType,
    "EnumeratedArgumentType": _w_EnumeratedArgumentType,
    "IntegerArgumentType": _w_IntegerArgumentType,
    "BinaryArgumentType": _w_BinaryArgumentType,
    "FloatArgumentType": _w_FloatArgumentType,
    "BooleanArgumentType": _w_BooleanArgumentType,
    "RelativeTimeArgumentType": _w_RelativeTimeArgumentType,
    "AbsoluteTimeArgumentType": _w_AbsoluteTimeArgumentType,
    "ArrayArgumentType": _w_ArrayArgumentType,
    "AggregateArgumentType": _w_AggregateArgumentType}


def _w_ArgumentTypeSetChoice(w: X.Writer, v: ArgumentTypeSetChoice) -> None:
    fn = _WC_ArgumentTypeSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentTypeSetChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentVariableStringChoice1 = {
    "DynamicValue": _w_ArgumentDynamicValueType,
    "DiscreteLookupList": _w_ArgumentDiscreteLookupListType}


def _w_ArgumentVariableStringChoice1(w: X.Writer,
                                     v: ArgumentVariableStringChoice1) -> None:
    fn = _WC_ArgumentVariableStringChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentVariableStringChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ArgumentVariableStringChoice2 = {
    "LeadingSize": _w_LeadingSizeType,
    "TerminationChar": lambda w, x, t: w.leaf(t, x)}


def _w_ArgumentVariableStringChoice2(w: X.Writer,
                                     v: ArgumentVariableStringChoice2) -> None:
    fn = _WC_ArgumentVariableStringChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("ArgumentVariableStringChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BaseDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                      "FloatDataEncoding": _w_FloatDataEncodingType,
                      "IntegerDataEncoding": _w_IntegerDataEncodingType,
                      "StringDataEncoding": _w_StringDataEncodingType}


def _w_BaseDataChoice(w: X.Writer, v: BaseDataChoice) -> None:
    fn = _WC_BaseDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"BaseDataChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_BinaryAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                         "CustomAlarm": _w_CustomAlarmType}


def _w_BinaryAlarmChoice(w: X.Writer, v: BinaryAlarmChoice) -> None:
    fn = _WC_BinaryAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"BinaryAlarmChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_BinaryArgumentChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_BinaryArgumentChoice(w: X.Writer, v: BinaryArgumentChoice) -> None:
    fn = _WC_BinaryArgumentChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BinaryArgumentChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BinaryContextAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                                "CustomAlarm": _w_CustomAlarmType}


def _w_BinaryContextAlarmChoice(w: X.Writer,
                                v: BinaryContextAlarmChoice) -> None:
    fn = _WC_BinaryContextAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BinaryContextAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BinaryDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                        "FloatDataEncoding": _w_FloatDataEncodingType,
                        "IntegerDataEncoding": _w_IntegerDataEncodingType,
                        "StringDataEncoding": _w_StringDataEncodingType}


def _w_BinaryDataChoice(w: X.Writer, v: BinaryDataChoice) -> None:
    fn = _WC_BinaryDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"BinaryDataChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_BinaryParameterChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                             "FloatDataEncoding": _w_FloatDataEncodingType,
                             "IntegerDataEncoding": _w_IntegerDataEncodingType,
                             "StringDataEncoding": _w_StringDataEncodingType}


def _w_BinaryParameterChoice(w: X.Writer, v: BinaryParameterChoice) -> None:
    fn = _WC_BinaryParameterChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BinaryParameterChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BooleanAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                          "CustomAlarm": _w_CustomAlarmType}


def _w_BooleanAlarmChoice(w: X.Writer, v: BooleanAlarmChoice) -> None:
    fn = _WC_BooleanAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"BooleanAlarmChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_BooleanArgumentChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_BooleanArgumentChoice(w: X.Writer, v: BooleanArgumentChoice) -> None:
    fn = _WC_BooleanArgumentChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BooleanArgumentChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BooleanContextAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                                 "CustomAlarm": _w_CustomAlarmType}


def _w_BooleanContextAlarmChoice(w: X.Writer,
                                 v: BooleanContextAlarmChoice) -> None:
    fn = _WC_BooleanContextAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BooleanContextAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BooleanDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                         "FloatDataEncoding": _w_FloatDataEncodingType,
                         "IntegerDataEncoding": _w_IntegerDataEncodingType,
                         "StringDataEncoding": _w_StringDataEncodingType}


def _w_BooleanDataChoice(w: X.Writer, v: BooleanDataChoice) -> None:
    fn = _WC_BooleanDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"BooleanDataChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_BooleanExpressionChoice = {"Condition": _w_ComparisonCheckType,
                               "ANDedConditions": _w_ANDedConditionsType,
                               "ORedConditions": _w_ORedConditionsType}


def _w_BooleanExpressionChoice(w: X.Writer, v: BooleanExpressionChoice) -> None:
    fn = _WC_BooleanExpressionChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BooleanExpressionChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_BooleanParameterChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                              "FloatDataEncoding": _w_FloatDataEncodingType,
                              "IntegerDataEncoding": _w_IntegerDataEncodingType,
                              "StringDataEncoding": _w_StringDataEncodingType}


def _w_BooleanParameterChoice(w: X.Writer, v: BooleanParameterChoice) -> None:
    fn = _WC_BooleanParameterChoice.get(v.name)
    if fn is None:
        raise X.XtceError("BooleanParameterChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_CalibratorChoice = {
    "SplineCalibrator": _w_SplineCalibratorType,
    "PolynomialCalibrator": _w_PolynomialCalibratorType,
    "MathOperationCalibrator": _w_MathOperationCalibratorType}


def _w_CalibratorChoice(w: X.Writer, v: CalibratorChoice) -> None:
    fn = _WC_CalibratorChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"CalibratorChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_CommandContainerEntryListChoice = {
    "ParameterRefEntry": _w_ArgumentParameterRefEntryType,
    "ParameterSegmentRefEntry": _w_ArgumentParameterSegmentRefEntryType,
    "ContainerRefEntry": _w_ArgumentContainerRefEntryType,
    "ContainerSegmentRefEntry": _w_ArgumentContainerSegmentRefEntryType,
    "StreamSegmentEntry": _w_ArgumentStreamSegmentEntryType,
    "IndirectParameterRefEntry": _w_ArgumentIndirectParameterRefEntryType,
    "ArrayParameterRefEntry": _w_ArgumentArrayParameterRefEntryType,
    "ArgumentRefEntry": _w_ArgumentArgumentRefEntryType,
    "ArrayArgumentRefEntry": _w_ArgumentArrayArgumentRefEntryType,
    "FixedValueEntry": _w_ArgumentFixedValueEntryType}


def _w_CommandContainerEntryListChoice(
        w: X.Writer, v: CommandContainerEntryListChoice) -> None:
    fn = _WC_CommandContainerEntryListChoice.get(v.name)
    if fn is None:
        raise X.XtceError("CommandContainerEntryListChoice: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_CommandVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_CommandVerifierChoice1(w: X.Writer, v: CommandVerifierChoice1) -> None:
    fn = _WC_CommandVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("CommandVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_CommandVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_CommandVerifierChoice2(w: X.Writer, v: CommandVerifierChoice2) -> None:
    fn = _WC_CommandVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("CommandVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ComparisonCheckChoice = {
    "ParameterInstanceRef": _w_ParameterInstanceRefType,
    "Value": lambda w, x, t: w.leaf(t, x)}


def _w_ComparisonCheckChoice(w: X.Writer, v: ComparisonCheckChoice) -> None:
    fn = _WC_ComparisonCheckChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ComparisonCheckChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_CompleteVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_CompleteVerifierChoice1(w: X.Writer, v: CompleteVerifierChoice1) -> None:
    fn = _WC_CompleteVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("CompleteVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_CompleteVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_CompleteVerifierChoice2(w: X.Writer, v: CompleteVerifierChoice2) -> None:
    fn = _WC_CompleteVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("CompleteVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ContextMatchChoice = {"Comparison": _w_ComparisonType,
                          "ComparisonList": _w_ComparisonListType,
                          "BooleanExpression": _w_BooleanExpressionType,
                          "CustomAlgorithm": _w_InputAlgorithmType}


def _w_ContextMatchChoice(w: X.Writer, v: ContextMatchChoice) -> None:
    fn = _WC_ContextMatchChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"ContextMatchChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_DiscreteLookupChoice = {"Comparison": _w_ComparisonType,
                            "ComparisonList": _w_ComparisonListType,
                            "BooleanExpression": _w_BooleanExpressionType,
                            "CustomAlgorithm": _w_InputAlgorithmType}


def _w_DiscreteLookupChoice(w: X.Writer, v: DiscreteLookupChoice) -> None:
    fn = _WC_DiscreteLookupChoice.get(v.name)
    if fn is None:
        raise X.XtceError("DiscreteLookupChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_EncodingChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                      "FloatDataEncoding": _w_FloatDataEncodingType,
                      "IntegerDataEncoding": _w_IntegerDataEncodingType,
                      "StringDataEncoding": _w_StringDataEncodingType}


def _w_EncodingChoice(w: X.Writer, v: EncodingChoice) -> None:
    fn = _WC_EncodingChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"EncodingChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_EntryListChoice = {
    "ParameterRefEntry": _w_ParameterRefEntryType,
    "ParameterSegmentRefEntry": _w_ParameterSegmentRefEntryType,
    "ContainerRefEntry": _w_ContainerRefEntryType,
    "ContainerSegmentRefEntry": _w_ContainerSegmentRefEntryType,
    "StreamSegmentEntry": _w_StreamSegmentEntryType,
    "IndirectParameterRefEntry": _w_IndirectParameterRefEntryType,
    "ArrayParameterRefEntry": _w_ArrayParameterRefEntryType}


def _w_EntryListChoice(w: X.Writer, v: EntryListChoice) -> None:
    fn = _WC_EntryListChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"EntryListChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_EnumeratedArgumentChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_EnumeratedArgumentChoice(w: X.Writer,
                                v: EnumeratedArgumentChoice) -> None:
    fn = _WC_EnumeratedArgumentChoice.get(v.name)
    if fn is None:
        raise X.XtceError("EnumeratedArgumentChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_EnumeratedDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                            "FloatDataEncoding": _w_FloatDataEncodingType,
                            "IntegerDataEncoding": _w_IntegerDataEncodingType,
                            "StringDataEncoding": _w_StringDataEncodingType}


def _w_EnumeratedDataChoice(w: X.Writer, v: EnumeratedDataChoice) -> None:
    fn = _WC_EnumeratedDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError("EnumeratedDataChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_EnumeratedParameterChoice = {
    "BinaryDataEncoding": _w_BinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_StringDataEncodingType}


def _w_EnumeratedParameterChoice(w: X.Writer,
                                 v: EnumeratedParameterChoice) -> None:
    fn = _WC_EnumeratedParameterChoice.get(v.name)
    if fn is None:
        raise X.XtceError("EnumeratedParameterChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_EnumerationAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                              "CustomAlarm": _w_CustomAlarmType}


def _w_EnumerationAlarmChoice(w: X.Writer, v: EnumerationAlarmChoice) -> None:
    fn = _WC_EnumerationAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("EnumerationAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_EnumerationContextAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                                     "CustomAlarm": _w_CustomAlarmType}


def _w_EnumerationContextAlarmChoice(w: X.Writer,
                                     v: EnumerationContextAlarmChoice) -> None:
    fn = _WC_EnumerationContextAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("EnumerationContextAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ErrorDetectCorrectChoice = {"Checksum": _w_ChecksumType, "CRC": _w_CRCType,
                                "XOR": _w_XORType, "Parity": _w_ParityType}


def _w_ErrorDetectCorrectChoice(w: X.Writer,
                                v: ErrorDetectCorrectChoice) -> None:
    fn = _WC_ErrorDetectCorrectChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ErrorDetectCorrectChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ExecutionVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_ExecutionVerifierChoice1(w: X.Writer,
                                v: ExecutionVerifierChoice1) -> None:
    fn = _WC_ExecutionVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("ExecutionVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ExecutionVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_ExecutionVerifierChoice2(w: X.Writer,
                                v: ExecutionVerifierChoice2) -> None:
    fn = _WC_ExecutionVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("ExecutionVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_FailedVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_FailedVerifierChoice1(w: X.Writer, v: FailedVerifierChoice1) -> None:
    fn = _WC_FailedVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("FailedVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_FailedVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_FailedVerifierChoice2(w: X.Writer, v: FailedVerifierChoice2) -> None:
    fn = _WC_FailedVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("FailedVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_FixedFrameStreamChoice = {"ContainerRef": _w_ContainerRefType,
                              "ServiceRef": _w_ServiceRefType}


def _w_FixedFrameStreamChoice(w: X.Writer, v: FixedFrameStreamChoice) -> None:
    fn = _WC_FixedFrameStreamChoice.get(v.name)
    if fn is None:
        raise X.XtceError("FixedFrameStreamChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_FloatArgumentChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_FloatArgumentChoice(w: X.Writer, v: FloatArgumentChoice) -> None:
    fn = _WC_FloatArgumentChoice.get(v.name)
    if fn is None:
        raise X.XtceError("FloatArgumentChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_FloatDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                       "FloatDataEncoding": _w_FloatDataEncodingType,
                       "IntegerDataEncoding": _w_IntegerDataEncodingType,
                       "StringDataEncoding": _w_StringDataEncodingType}


def _w_FloatDataChoice(w: X.Writer, v: FloatDataChoice) -> None:
    fn = _WC_FloatDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"FloatDataChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_FloatParameterChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                            "FloatDataEncoding": _w_FloatDataEncodingType,
                            "IntegerDataEncoding": _w_IntegerDataEncodingType,
                            "StringDataEncoding": _w_StringDataEncodingType}


def _w_FloatParameterChoice(w: X.Writer, v: FloatParameterChoice) -> None:
    fn = _WC_FloatParameterChoice.get(v.name)
    if fn is None:
        raise X.XtceError("FloatParameterChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_FrameStreamChoice = {"ContainerRef": _w_ContainerRefType,
                         "ServiceRef": _w_ServiceRefType}


def _w_FrameStreamChoice(w: X.Writer, v: FrameStreamChoice) -> None:
    fn = _WC_FrameStreamChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"FrameStreamChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_InputSetChoice = {
    "InputParameterInstanceRef": _w_InputParameterInstanceRefType,
    "Constant": _w_ConstantType}


def _w_InputSetChoice(w: X.Writer, v: InputSetChoice) -> None:
    fn = _WC_InputSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"InputSetChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_IntegerArgumentChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_IntegerArgumentChoice(w: X.Writer, v: IntegerArgumentChoice) -> None:
    fn = _WC_IntegerArgumentChoice.get(v.name)
    if fn is None:
        raise X.XtceError("IntegerArgumentChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_IntegerDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                         "FloatDataEncoding": _w_FloatDataEncodingType,
                         "IntegerDataEncoding": _w_IntegerDataEncodingType,
                         "StringDataEncoding": _w_StringDataEncodingType}


def _w_IntegerDataChoice(w: X.Writer, v: IntegerDataChoice) -> None:
    fn = _WC_IntegerDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"IntegerDataChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_IntegerParameterChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                              "FloatDataEncoding": _w_FloatDataEncodingType,
                              "IntegerDataEncoding": _w_IntegerDataEncodingType,
                              "StringDataEncoding": _w_StringDataEncodingType}


def _w_IntegerParameterChoice(w: X.Writer, v: IntegerParameterChoice) -> None:
    fn = _WC_IntegerParameterChoice.get(v.name)
    if fn is None:
        raise X.XtceError("IntegerParameterChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_IntegerValueChoice = {"FixedValue": lambda w, x, t: w.leaf(t, X.f_int(x)),
                          "DynamicValue": _w_DynamicValueType,
                          "DiscreteLookupList": _w_DiscreteLookupListType}


def _w_IntegerValueChoice(w: X.Writer, v: IntegerValueChoice) -> None:
    fn = _WC_IntegerValueChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"IntegerValueChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_LocationInContainerInBitsChoice = {
    "FixedValue": lambda w, x, t: w.leaf(t, X.f_int(x)),
    "DynamicValue": _w_DynamicValueType,
    "DiscreteLookupList": _w_DiscreteLookupListType}


def _w_LocationInContainerInBitsChoice(
        w: X.Writer, v: LocationInContainerInBitsChoice) -> None:
    fn = _WC_LocationInContainerInBitsChoice.get(v.name)
    if fn is None:
        raise X.XtceError("LocationInContainerInBitsChoice: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_MatchCriteriaChoice = {"Comparison": _w_ComparisonType,
                           "ComparisonList": _w_ComparisonListType,
                           "BooleanExpression": _w_BooleanExpressionType,
                           "CustomAlgorithm": _w_InputAlgorithmType}


def _w_MatchCriteriaChoice(w: X.Writer, v: MatchCriteriaChoice) -> None:
    fn = _WC_MatchCriteriaChoice.get(v.name)
    if fn is None:
        raise X.XtceError("MatchCriteriaChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_MathOperationCalibratorChoice = {
    "ValueOperand": lambda w, x, t: w.leaf(t, x),
    "ThisParameterOperand": lambda w, x, t: w.leaf(t, x),
    "Operator": lambda w, x, t: w.leaf(t, x.value),
    "ParameterInstanceRefOperand": _w_ParameterInstanceRefType}


def _w_MathOperationCalibratorChoice(w: X.Writer,
                                     v: MathOperationCalibratorChoice) -> None:
    fn = _WC_MathOperationCalibratorChoice.get(v.name)
    if fn is None:
        raise X.XtceError("MathOperationCalibratorChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_MathOperationChoice = {
    "ValueOperand": lambda w, x, t: w.leaf(t, x),
    "ThisParameterOperand": lambda w, x, t: w.leaf(t, x),
    "Operator": lambda w, x, t: w.leaf(t, x.value),
    "ParameterInstanceRefOperand": _w_ParameterInstanceRefType}


def _w_MathOperationChoice(w: X.Writer, v: MathOperationChoice) -> None:
    fn = _WC_MathOperationChoice.get(v.name)
    if fn is None:
        raise X.XtceError("MathOperationChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_MetaCommandSetChoice = {"MetaCommand": _w_MetaCommandType,
                            "MetaCommandRef": lambda w, x, t: w.leaf(t, x),
                            "BlockMetaCommand": _w_BlockMetaCommandType}


def _w_MetaCommandSetChoice(w: X.Writer, v: MetaCommandSetChoice) -> None:
    fn = _WC_MetaCommandSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError("MetaCommandSetChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_NumericAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                          "CustomAlarm": _w_CustomAlarmType}


def _w_NumericAlarmChoice(w: X.Writer, v: NumericAlarmChoice) -> None:
    fn = _WC_NumericAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"NumericAlarmChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_NumericContextAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                                 "CustomAlarm": _w_CustomAlarmType}


def _w_NumericContextAlarmChoice(w: X.Writer,
                                 v: NumericContextAlarmChoice) -> None:
    fn = _WC_NumericContextAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("NumericContextAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ORedConditionsChoice = {"Condition": _w_ComparisonCheckType,
                            "ANDedConditions": _w_ANDedConditionsType}


def _w_ORedConditionsChoice(w: X.Writer, v: ORedConditionsChoice) -> None:
    fn = _WC_ORedConditionsChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ORedConditionsChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ParameterSetChoice = {"Parameter": _w_ParameterType,
                          "ParameterRef": _w_ParameterRefType}


def _w_ParameterSetChoice(w: X.Writer, v: ParameterSetChoice) -> None:
    fn = _WC_ParameterSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"ParameterSetChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_ParameterToSetChoice = {"Derivation": _w_ArgumentMathOperationType,
                            "NewValue": lambda w, x, t: w.leaf(t, x)}


def _w_ParameterToSetChoice(w: X.Writer, v: ParameterToSetChoice) -> None:
    fn = _WC_ParameterToSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ParameterToSetChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ParameterTypeSetChoice = {
    "StringParameterType": _w_StringParameterType,
    "EnumeratedParameterType": _w_EnumeratedParameterType,
    "IntegerParameterType": _w_IntegerParameterType,
    "BinaryParameterType": _w_BinaryParameterType,
    "FloatParameterType": _w_FloatParameterType,
    "BooleanParameterType": _w_BooleanParameterType,
    "RelativeTimeParameterType": _w_RelativeTimeParameterType,
    "AbsoluteTimeParameterType": _w_AbsoluteTimeParameterType,
    "ArrayParameterType": _w_ArrayParameterType,
    "AggregateParameterType": _w_AggregateParameterType}


def _w_ParameterTypeSetChoice(w: X.Writer, v: ParameterTypeSetChoice) -> None:
    fn = _WC_ParameterTypeSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ParameterTypeSetChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_PercentCompleteChoice = {
    "FixedValue": lambda w, x, t: w.leaf(t, X.f_double(x)),
    "DynamicValue": _w_DynamicValueType}


def _w_PercentCompleteChoice(w: X.Writer, v: PercentCompleteChoice) -> None:
    fn = _WC_PercentCompleteChoice.get(v.name)
    if fn is None:
        raise X.XtceError("PercentCompleteChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_QueuedVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_QueuedVerifierChoice1(w: X.Writer, v: QueuedVerifierChoice1) -> None:
    fn = _WC_QueuedVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("QueuedVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_QueuedVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_QueuedVerifierChoice2(w: X.Writer, v: QueuedVerifierChoice2) -> None:
    fn = _WC_QueuedVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("QueuedVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ReceivedVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_ReceivedVerifierChoice1(w: X.Writer, v: ReceivedVerifierChoice1) -> None:
    fn = _WC_ReceivedVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("ReceivedVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ReceivedVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_ReceivedVerifierChoice2(w: X.Writer, v: ReceivedVerifierChoice2) -> None:
    fn = _WC_ReceivedVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("ReceivedVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ReferenceTimeChoice = {"OffsetFrom": _w_ParameterInstanceRefType,
                           "Epoch": lambda w, x, t: w.leaf(t, x)}


def _w_ReferenceTimeChoice(w: X.Writer, v: ReferenceTimeChoice) -> None:
    fn = _WC_ReferenceTimeChoice.get(v.name)
    if fn is None:
        raise X.XtceError("ReferenceTimeChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_RestrictionCriteriaChoice = {"Comparison": _w_ComparisonType,
                                 "ComparisonList": _w_ComparisonListType,
                                 "BooleanExpression": _w_BooleanExpressionType,
                                 "CustomAlgorithm": _w_InputAlgorithmType}


def _w_RestrictionCriteriaChoice(w: X.Writer,
                                 v: RestrictionCriteriaChoice) -> None:
    fn = _WC_RestrictionCriteriaChoice.get(v.name)
    if fn is None:
        raise X.XtceError("RestrictionCriteriaChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_SentFromRangeVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_SentFromRangeVerifierChoice1(w: X.Writer,
                                    v: SentFromRangeVerifierChoice1) -> None:
    fn = _WC_SentFromRangeVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("SentFromRangeVerifierChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_SentFromRangeVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_SentFromRangeVerifierChoice2(w: X.Writer,
                                    v: SentFromRangeVerifierChoice2) -> None:
    fn = _WC_SentFromRangeVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("SentFromRangeVerifierChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_ServiceChoice = {"MessageRefSet": _w_MessageRefSetType,
                     "ContainerRefSet": _w_ContainerRefSetType}


def _w_ServiceChoice(w: X.Writer, v: ServiceChoice) -> None:
    fn = _WC_ServiceChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"ServiceChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_StreamSetChoice = {"FixedFrameStream": _w_FixedFrameStreamType,
                       "VariableFrameStream": _w_VariableFrameStreamType,
                       "CustomStream": _w_CustomStreamType}


def _w_StreamSetChoice(w: X.Writer, v: StreamSetChoice) -> None:
    fn = _WC_StreamSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"StreamSetChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_StringAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                         "CustomAlarm": _w_CustomAlarmType}


def _w_StringAlarmChoice(w: X.Writer, v: StringAlarmChoice) -> None:
    fn = _WC_StringAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"StringAlarmChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_StringArgumentChoice = {
    "BinaryDataEncoding": _w_ArgumentBinaryDataEncodingType,
    "FloatDataEncoding": _w_FloatDataEncodingType,
    "IntegerDataEncoding": _w_IntegerDataEncodingType,
    "StringDataEncoding": _w_ArgumentStringDataEncodingType}


def _w_StringArgumentChoice(w: X.Writer, v: StringArgumentChoice) -> None:
    fn = _WC_StringArgumentChoice.get(v.name)
    if fn is None:
        raise X.XtceError("StringArgumentChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_StringContextAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                                "CustomAlarm": _w_CustomAlarmType}


def _w_StringContextAlarmChoice(w: X.Writer,
                                v: StringContextAlarmChoice) -> None:
    fn = _WC_StringContextAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("StringContextAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_StringDataChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                        "FloatDataEncoding": _w_FloatDataEncodingType,
                        "IntegerDataEncoding": _w_IntegerDataEncodingType,
                        "StringDataEncoding": _w_StringDataEncodingType}


def _w_StringDataChoice(w: X.Writer, v: StringDataChoice) -> None:
    fn = _WC_StringDataChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"StringDataChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_StringDataEncodingChoice = {"SizeInBits": _w_SizeInBitsType,
                                "Variable": _w_VariableStringType}


def _w_StringDataEncodingChoice(w: X.Writer,
                                v: StringDataEncodingChoice) -> None:
    fn = _WC_StringDataEncodingChoice.get(v.name)
    if fn is None:
        raise X.XtceError("StringDataEncodingChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_StringParameterChoice = {"BinaryDataEncoding": _w_BinaryDataEncodingType,
                             "FloatDataEncoding": _w_FloatDataEncodingType,
                             "IntegerDataEncoding": _w_IntegerDataEncodingType,
                             "StringDataEncoding": _w_StringDataEncodingType}


def _w_StringParameterChoice(w: X.Writer, v: StringParameterChoice) -> None:
    fn = _WC_StringParameterChoice.get(v.name)
    if fn is None:
        raise X.XtceError("StringParameterChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_TimeAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                       "CustomAlarm": _w_CustomAlarmType}


def _w_TimeAlarmChoice(w: X.Writer, v: TimeAlarmChoice) -> None:
    fn = _WC_TimeAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"TimeAlarmChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_TimeContextAlarmChoice = {"AlarmConditions": _w_AlarmConditionsType,
                              "CustomAlarm": _w_CustomAlarmType}


def _w_TimeContextAlarmChoice(w: X.Writer, v: TimeContextAlarmChoice) -> None:
    fn = _WC_TimeContextAlarmChoice.get(v.name)
    if fn is None:
        raise X.XtceError("TimeContextAlarmChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_TransferredToRangeVerifierChoice1 = {
    "ComparisonList": _w_ComparisonListType,
    "ContainerRef": _w_ContainerRefType,
    "ParameterValueChange": _w_ParameterValueChangeType,
    "CustomAlgorithm": _w_InputAlgorithmType,
    "BooleanExpression": _w_BooleanExpressionType,
    "Comparison": _w_ComparisonType}


def _w_TransferredToRangeVerifierChoice1(
        w: X.Writer, v: TransferredToRangeVerifierChoice1) -> None:
    fn = _WC_TransferredToRangeVerifierChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("TransferredToRangeVerifierChoice1: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_TransferredToRangeVerifierChoice2 = {
    "CheckWindow": _w_CheckWindowType,
    "CheckWindowAlgorithms": _w_CheckWindowAlgorithmsType}


def _w_TransferredToRangeVerifierChoice2(
        w: X.Writer, v: TransferredToRangeVerifierChoice2) -> None:
    fn = _WC_TransferredToRangeVerifierChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("TransferredToRangeVerifierChoice2: unknown "
                          f"alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_TransmissionConstraintChoice = {
    "Comparison": _w_ComparisonType, "ComparisonList": _w_ComparisonListType,
    "BooleanExpression": _w_BooleanExpressionType,
    "CustomAlgorithm": _w_InputAlgorithmType}


def _w_TransmissionConstraintChoice(w: X.Writer,
                                    v: TransmissionConstraintChoice) -> None:
    fn = _WC_TransmissionConstraintChoice.get(v.name)
    if fn is None:
        raise X.XtceError("TransmissionConstraintChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_TriggerSetChoice = {
    "OnParameterUpdateTrigger": _w_OnParameterUpdateTriggerType,
    "OnContainerUpdateTrigger": _w_OnContainerUpdateTriggerType,
    "OnPeriodicRateTrigger": _w_OnPeriodicRateTriggerType}


def _w_TriggerSetChoice(w: X.Writer, v: TriggerSetChoice) -> None:
    fn = _WC_TriggerSetChoice.get(v.name)
    if fn is None:
        raise X.XtceError(f"TriggerSetChoice: unknown alternative {v.name!r}")
    fn(w, v.value, v.name)


_WC_TriggeredMathOperationChoice = {
    "ValueOperand": lambda w, x, t: w.leaf(t, x),
    "ThisParameterOperand": lambda w, x, t: w.leaf(t, x),
    "Operator": lambda w, x, t: w.leaf(t, x.value),
    "ParameterInstanceRefOperand": _w_ParameterInstanceRefType}


def _w_TriggeredMathOperationChoice(w: X.Writer,
                                    v: TriggeredMathOperationChoice) -> None:
    fn = _WC_TriggeredMathOperationChoice.get(v.name)
    if fn is None:
        raise X.XtceError("TriggeredMathOperationChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_VariableFrameStreamChoice = {"ContainerRef": _w_ContainerRefType,
                                 "ServiceRef": _w_ServiceRefType}


def _w_VariableFrameStreamChoice(w: X.Writer,
                                 v: VariableFrameStreamChoice) -> None:
    fn = _WC_VariableFrameStreamChoice.get(v.name)
    if fn is None:
        raise X.XtceError("VariableFrameStreamChoice: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_VariableStringChoice1 = {"DynamicValue": _w_DynamicValueType,
                             "DiscreteLookupList": _w_DiscreteLookupListType}


def _w_VariableStringChoice1(w: X.Writer, v: VariableStringChoice1) -> None:
    fn = _WC_VariableStringChoice1.get(v.name)
    if fn is None:
        raise X.XtceError("VariableStringChoice1: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)


_WC_VariableStringChoice2 = {"LeadingSize": _w_LeadingSizeType,
                             "TerminationChar": lambda w, x, t: w.leaf(t, x)}


def _w_VariableStringChoice2(w: X.Writer, v: VariableStringChoice2) -> None:
    fn = _WC_VariableStringChoice2.get(v.name)
    if fn is None:
        raise X.XtceError("VariableStringChoice2: unknown alternative "
                          f"{v.name!r}")
    fn(w, v.value, v.name)

READERS = {
    ANDedConditionsType: _r_ANDedConditionsType,
    AbsoluteTimeArgumentType: _r_AbsoluteTimeArgumentType,
    AbsoluteTimeDataType: _r_AbsoluteTimeDataType,
    AbsoluteTimeParameterType: _r_AbsoluteTimeParameterType,
    AcceptedVerifierType: _r_AcceptedVerifierType,
    AggregateArgumentType: _r_AggregateArgumentType,
    AggregateDataType: _r_AggregateDataType,
    AggregateParameterType: _r_AggregateParameterType,
    AlarmConditionsType: _r_AlarmConditionsType,
    AlarmMultiRangesType: _r_AlarmMultiRangesType,
    AlarmRangesType: _r_AlarmRangesType,
    AlarmType: _r_AlarmType,
    AlgorithmSetType: _r_AlgorithmSetType,
    AlgorithmTextType: _r_AlgorithmTextType,
    AliasSetType: _r_AliasSetType,
    AliasType: _r_AliasType,
    AncillaryDataSetType: _r_AncillaryDataSetType,
    AncillaryDataType: _r_AncillaryDataType,
    ArgumentANDedConditionsType: _r_ArgumentANDedConditionsType,
    ArgumentAbsoluteTimeDataType: _r_ArgumentAbsoluteTimeDataType,
    ArgumentArgumentRefEntryType: _r_ArgumentArgumentRefEntryType,
    ArgumentArrayArgumentRefEntryType: _r_ArgumentArrayArgumentRefEntryType,
    ArgumentArrayParameterRefEntryType: _r_ArgumentArrayParameterRefEntryType,
    ArgumentAssignmentListType: _r_ArgumentAssignmentListType,
    ArgumentAssignmentType: _r_ArgumentAssignmentType,
    ArgumentBaseDataType: _r_ArgumentBaseDataType,
    ArgumentBaseTimeDataType: _r_ArgumentBaseTimeDataType,
    ArgumentBinaryDataEncodingType: _r_ArgumentBinaryDataEncodingType,
    ArgumentBinaryDataType: _r_ArgumentBinaryDataType,
    ArgumentBooleanDataType: _r_ArgumentBooleanDataType,
    ArgumentBooleanExpressionType: _r_ArgumentBooleanExpressionType,
    ArgumentComparisonCheckType: _r_ArgumentComparisonCheckType,
    ArgumentComparisonListType: _r_ArgumentComparisonListType,
    ArgumentComparisonType: _r_ArgumentComparisonType,
    ArgumentContainerRefEntryType: _r_ArgumentContainerRefEntryType,
    ArgumentContainerSegmentRefEntryType:
            _r_ArgumentContainerSegmentRefEntryType,
    ArgumentDimensionListType: _r_ArgumentDimensionListType,
    ArgumentDimensionType: _r_ArgumentDimensionType,
    ArgumentDiscreteLookupListType: _r_ArgumentDiscreteLookupListType,
    ArgumentDiscreteLookupType: _r_ArgumentDiscreteLookupType,
    ArgumentDynamicValueType: _r_ArgumentDynamicValueType,
    ArgumentEnumeratedDataType: _r_ArgumentEnumeratedDataType,
    ArgumentFixedValueEntryType: _r_ArgumentFixedValueEntryType,
    ArgumentFloatDataType: _r_ArgumentFloatDataType,
    ArgumentIndirectParameterRefEntryType:
            _r_ArgumentIndirectParameterRefEntryType,
    ArgumentInputAlgorithmType: _r_ArgumentInputAlgorithmType,
    ArgumentInputSetType: _r_ArgumentInputSetType,
    ArgumentInstanceRefType: _r_ArgumentInstanceRefType,
    ArgumentIntegerDataType: _r_ArgumentIntegerDataType,
    ArgumentIntegerValueType: _r_ArgumentIntegerValueType,
    ArgumentListType: _r_ArgumentListType,
    ArgumentLocationInContainerInBitsType:
            _r_ArgumentLocationInContainerInBitsType,
    ArgumentMatchCriteriaType: _r_ArgumentMatchCriteriaType,
    ArgumentMathOperationType: _r_ArgumentMathOperationType,
    ArgumentORedConditionsType: _r_ArgumentORedConditionsType,
    ArgumentParameterRefEntryType: _r_ArgumentParameterRefEntryType,
    ArgumentParameterSegmentRefEntryType:
            _r_ArgumentParameterSegmentRefEntryType,
    ArgumentRelativeTimeDataType: _r_ArgumentRelativeTimeDataType,
    ArgumentRepeatType: _r_ArgumentRepeatType,
    ArgumentRestrictionListType: _r_ArgumentRestrictionListType,
    ArgumentSequenceEntryType: _r_ArgumentSequenceEntryType,
    ArgumentStreamSegmentEntryType: _r_ArgumentStreamSegmentEntryType,
    ArgumentStringDataEncodingType: _r_ArgumentStringDataEncodingType,
    ArgumentStringDataType: _r_ArgumentStringDataType,
    ArgumentType: _r_ArgumentType,
    ArgumentTypeSetType: _r_ArgumentTypeSetType,
    ArgumentVariableStringType: _r_ArgumentVariableStringType,
    ArrayArgumentType: _r_ArrayArgumentType,
    ArrayDataTypeType: _r_ArrayDataTypeType,
    ArrayParameterRefEntryType: _r_ArrayParameterRefEntryType,
    ArrayParameterType: _r_ArrayParameterType,
    AuthorSetType: _r_AuthorSetType,
    AutoInvertType: _r_AutoInvertType,
    BaseAlarmType: _r_BaseAlarmType,
    BaseCalibratorType: _r_BaseCalibratorType,
    BaseComparisonType: _r_BaseComparisonType,
    BaseConditionsType: _r_BaseConditionsType,
    BaseContainerType: _r_BaseContainerType,
    BaseDataType: _r_BaseDataType,
    BaseMetaCommandType: _r_BaseMetaCommandType,
    BaseTimeDataType: _r_BaseTimeDataType,
    BaseTriggerType: _r_BaseTriggerType,
    BinaryAlarmType: _r_BinaryAlarmType,
    BinaryArgumentType: _r_BinaryArgumentType,
    BinaryContextAlarmListType: _r_BinaryContextAlarmListType,
    BinaryContextAlarmType: _r_BinaryContextAlarmType,
    BinaryDataEncodingType: _r_BinaryDataEncodingType,
    BinaryDataType: _r_BinaryDataType,
    BinaryParameterType: _r_BinaryParameterType,
    BlockMetaCommandType: _r_BlockMetaCommandType,
    BooleanAlarmType: _r_BooleanAlarmType,
    BooleanArgumentType: _r_BooleanArgumentType,
    BooleanContextAlarmListType: _r_BooleanContextAlarmListType,
    BooleanContextAlarmType: _r_BooleanContextAlarmType,
    BooleanDataType: _r_BooleanDataType,
    BooleanExpressionType: _r_BooleanExpressionType,
    BooleanParameterType: _r_BooleanParameterType,
    ByteType: _r_ByteType,
    CRCType: _r_CRCType,
    CalibratorType: _r_CalibratorType,
    ChangeAlarmRangesType: _r_ChangeAlarmRangesType,
    ChangeValueType: _r_ChangeValueType,
    CheckWindowAlgorithmsType: _r_CheckWindowAlgorithmsType,
    CheckWindowType: _r_CheckWindowType,
    ChecksumType: _r_ChecksumType,
    CommandContainerEntryListType: _r_CommandContainerEntryListType,
    CommandContainerSetType: _r_CommandContainerSetType,
    CommandContainerType: _r_CommandContainerType,
    CommandMetaDataType: _r_CommandMetaDataType,
    CommandVerifierType: _r_CommandVerifierType,
    ComparisonCheckType: _r_ComparisonCheckType,
    ComparisonListType: _r_ComparisonListType,
    ComparisonType: _r_ComparisonType,
    CompleteVerifierType: _r_CompleteVerifierType,
    ConstantType: _r_ConstantType,
    ContainerBinaryDataEncodingType: _r_ContainerBinaryDataEncodingType,
    ContainerRefEntryType: _r_ContainerRefEntryType,
    ContainerRefSetType: _r_ContainerRefSetType,
    ContainerRefType: _r_ContainerRefType,
    ContainerSegmentRefEntryType: _r_ContainerSegmentRefEntryType,
    ContainerSetType: _r_ContainerSetType,
    ContainerType: _r_ContainerType,
    ContextCalibratorListType: _r_ContextCalibratorListType,
    ContextCalibratorType: _r_ContextCalibratorType,
    ContextMatchType: _r_ContextMatchType,
    ContextSignificanceListType: _r_ContextSignificanceListType,
    ContextSignificanceType: _r_ContextSignificanceType,
    CustomAlarmType: _r_CustomAlarmType,
    CustomStreamType: _r_CustomStreamType,
    DataEncodingType: _r_DataEncodingType,
    DescriptionType: _r_DescriptionType,
    DimensionListType: _r_DimensionListType,
    DimensionType: _r_DimensionType,
    DiscreteLookupListType: _r_DiscreteLookupListType,
    DiscreteLookupType: _r_DiscreteLookupType,
    DynamicValueType: _r_DynamicValueType,
    EncodingType: _r_EncodingType,
    EntryListType: _r_EntryListType,
    EnumeratedArgumentType: _r_EnumeratedArgumentType,
    EnumeratedDataType: _r_EnumeratedDataType,
    EnumeratedParameterType: _r_EnumeratedParameterType,
    EnumerationAlarmLevelType: _r_EnumerationAlarmLevelType,
    EnumerationAlarmListType: _r_EnumerationAlarmListType,
    EnumerationAlarmType: _r_EnumerationAlarmType,
    EnumerationContextAlarmListType: _r_EnumerationContextAlarmListType,
    EnumerationContextAlarmType: _r_EnumerationContextAlarmType,
    EnumerationListType: _r_EnumerationListType,
    ErrorDetectCorrectType: _r_ErrorDetectCorrectType,
    ExecutionVerifierType: _r_ExecutionVerifierType,
    ExternalAlgorithmSetType: _r_ExternalAlgorithmSetType,
    ExternalAlgorithmType: _r_ExternalAlgorithmType,
    FailedVerifierType: _r_FailedVerifierType,
    FixedFrameStreamType: _r_FixedFrameStreamType,
    FixedFrameSyncStrategyType: _r_FixedFrameSyncStrategyType,
    FlagType: _r_FlagType,
    FloatArgumentType: _r_FloatArgumentType,
    FloatDataEncodingType: _r_FloatDataEncodingType,
    FloatDataType: _r_FloatDataType,
    FloatDataValidRangeType: _r_FloatDataValidRangeType,
    FloatParameterType: _r_FloatParameterType,
    FloatRangeType: _r_FloatRangeType,
    FrameStreamType: _r_FrameStreamType,
    HeaderType: _r_HeaderType,
    HistorySetType: _r_HistorySetType,
    IndirectParameterRefEntryType: _r_IndirectParameterRefEntryType,
    InputAlgorithmType: _r_InputAlgorithmType,
    InputOutputAlgorithmType: _r_InputOutputAlgorithmType,
    InputOutputTriggerAlgorithmType: _r_InputOutputTriggerAlgorithmType,
    InputParameterInstanceRefType: _r_InputParameterInstanceRefType,
    InputSetType: _r_InputSetType,
    IntegerArgumentType: _r_IntegerArgumentType,
    IntegerDataEncodingType: _r_IntegerDataEncodingType,
    IntegerDataType: _r_IntegerDataType,
    IntegerDataValidRangeType: _r_IntegerDataValidRangeType,
    IntegerParameterType: _r_IntegerParameterType,
    IntegerRangeType: _r_IntegerRangeType,
    IntegerValueType: _r_IntegerValueType,
    InterlockType: _r_InterlockType,
    LeadingSizeType: _r_LeadingSizeType,
    LinearAdjustmentType: _r_LinearAdjustmentType,
    LocationInContainerInBitsType: _r_LocationInContainerInBitsType,
    MatchCriteriaType: _r_MatchCriteriaType,
    MathAlgorithmType: _r_MathAlgorithmType,
    MathOperationCalibratorType: _r_MathOperationCalibratorType,
    MathOperationType: _r_MathOperationType,
    MemberListType: _r_MemberListType,
    MemberType: _r_MemberType,
    MessageRefSetType: _r_MessageRefSetType,
    MessageRefType: _r_MessageRefType,
    MessageSetType: _r_MessageSetType,
    MessageType: _r_MessageType,
    MetaCommandSetType: _r_MetaCommandSetType,
    MetaCommandStepListType: _r_MetaCommandStepListType,
    MetaCommandStepType: _r_MetaCommandStepType,
    MetaCommandType: _r_MetaCommandType,
    MultiRangeType: _r_MultiRangeType,
    NameDescriptionType: _r_NameDescriptionType,
    NoteSetType: _r_NoteSetType,
    NumberFormatType: _r_NumberFormatType,
    NumericAlarmType: _r_NumericAlarmType,
    NumericContextAlarmListType: _r_NumericContextAlarmListType,
    NumericContextAlarmType: _r_NumericContextAlarmType,
    ORedConditionsType: _r_ORedConditionsType,
    OnContainerUpdateTriggerType: _r_OnContainerUpdateTriggerType,
    OnParameterUpdateTriggerType: _r_OnParameterUpdateTriggerType,
    OnPeriodicRateTriggerType: _r_OnPeriodicRateTriggerType,
    OptionalNameDescriptionType: _r_OptionalNameDescriptionType,
    OutputParameterRefType: _r_OutputParameterRefType,
    OutputSetType: _r_OutputSetType,
    PCMStreamType: _r_PCMStreamType,
    ParameterInstanceRefType: _r_ParameterInstanceRefType,
    ParameterPropertiesType: _r_ParameterPropertiesType,
    ParameterRefEntryType: _r_ParameterRefEntryType,
    ParameterRefType: _r_ParameterRefType,
    ParameterSegmentRefEntryType: _r_ParameterSegmentRefEntryType,
    ParameterSetType: _r_ParameterSetType,
    ParameterToSetListType: _r_ParameterToSetListType,
    ParameterToSetType: _r_ParameterToSetType,
    ParameterToSuspendAlarmsOnType: _r_ParameterToSuspendAlarmsOnType,
    ParameterType: _r_ParameterType,
    ParameterTypeSetType: _r_ParameterTypeSetType,
    ParameterValueChangeType: _r_ParameterValueChangeType,
    ParametersToSuspendAlarmsOnSetType: _r_ParametersToSuspendAlarmsOnSetType,
    ParityType: _r_ParityType,
    PercentCompleteType: _r_PercentCompleteType,
    PhysicalAddressSetType: _r_PhysicalAddressSetType,
    PhysicalAddressType: _r_PhysicalAddressType,
    PolynomialCalibratorType: _r_PolynomialCalibratorType,
    QueuedVerifierType: _r_QueuedVerifierType,
    RateInStreamSetType: _r_RateInStreamSetType,
    RateInStreamType: _r_RateInStreamType,
    RateInStreamWithStreamNameType: _r_RateInStreamWithStreamNameType,
    ReceivedVerifierType: _r_ReceivedVerifierType,
    ReferenceTimeType: _r_ReferenceTimeType,
    RelativeTimeArgumentType: _r_RelativeTimeArgumentType,
    RelativeTimeDataType: _r_RelativeTimeDataType,
    RelativeTimeParameterType: _r_RelativeTimeParameterType,
    RepeatType: _r_RepeatType,
    RestrictionCriteriaType: _r_RestrictionCriteriaType,
    SentFromRangeVerifierType: _r_SentFromRangeVerifierType,
    SequenceContainerType: _r_SequenceContainerType,
    SequenceEntryType: _r_SequenceEntryType,
    ServiceRefType: _r_ServiceRefType,
    ServiceSetType: _r_ServiceSetType,
    ServiceType: _r_ServiceType,
    SignificanceType: _r_SignificanceType,
    SimpleAlgorithmType: _r_SimpleAlgorithmType,
    SizeInBitsFixedType: _r_SizeInBitsFixedType,
    SizeInBitsType: _r_SizeInBitsType,
    SpaceSystemType: _r_SpaceSystemType,
    SplineCalibratorType: _r_SplineCalibratorType,
    SplinePointType: _r_SplinePointType,
    StreamRefType: _r_StreamRefType,
    StreamSegmentEntryType: _r_StreamSegmentEntryType,
    StreamSetType: _r_StreamSetType,
    StringAlarmLevelType: _r_StringAlarmLevelType,
    StringAlarmListType: _r_StringAlarmListType,
    StringAlarmType: _r_StringAlarmType,
    StringArgumentType: _r_StringArgumentType,
    StringContextAlarmListType: _r_StringContextAlarmListType,
    StringContextAlarmType: _r_StringContextAlarmType,
    StringDataEncodingType: _r_StringDataEncodingType,
    StringDataType: _r_StringDataType,
    StringParameterType: _r_StringParameterType,
    SyncPatternType: _r_SyncPatternType,
    SyncStrategyType: _r_SyncStrategyType,
    TelemetryMetaDataType: _r_TelemetryMetaDataType,
    TermType: _r_TermType,
    TimeAlarmRangesType: _r_TimeAlarmRangesType,
    TimeAlarmType: _r_TimeAlarmType,
    TimeAssociationType: _r_TimeAssociationType,
    TimeContextAlarmListType: _r_TimeContextAlarmListType,
    TimeContextAlarmType: _r_TimeContextAlarmType,
    ToStringType: _r_ToStringType,
    TransferredToRangeVerifierType: _r_TransferredToRangeVerifierType,
    TransmissionConstraintListType: _r_TransmissionConstraintListType,
    TransmissionConstraintType: _r_TransmissionConstraintType,
    TriggerSetType: _r_TriggerSetType,
    TriggeredMathOperationType: _r_TriggeredMathOperationType,
    UnitSetType: _r_UnitSetType,
    UnitType: _r_UnitType,
    ValidFloatRangeSetType: _r_ValidFloatRangeSetType,
    ValidIntegerRangeSetType: _r_ValidIntegerRangeSetType,
    ValueEnumerationType: _r_ValueEnumerationType,
    VariableFrameStreamType: _r_VariableFrameStreamType,
    VariableFrameSyncStrategyType: _r_VariableFrameSyncStrategyType,
    VariableStringType: _r_VariableStringType,
    VerifierSetType: _r_VerifierSetType,
    XORType: _r_XORType,
}
WRITERS = {
    ANDedConditionsType: _w_ANDedConditionsType,
    AbsoluteTimeArgumentType: _w_AbsoluteTimeArgumentType,
    AbsoluteTimeDataType: _w_AbsoluteTimeDataType,
    AbsoluteTimeParameterType: _w_AbsoluteTimeParameterType,
    AcceptedVerifierType: _w_AcceptedVerifierType,
    AggregateArgumentType: _w_AggregateArgumentType,
    AggregateDataType: _w_AggregateDataType,
    AggregateParameterType: _w_AggregateParameterType,
    AlarmConditionsType: _w_AlarmConditionsType,
    AlarmMultiRangesType: _w_AlarmMultiRangesType,
    AlarmRangesType: _w_AlarmRangesType,
    AlarmType: _w_AlarmType,
    AlgorithmSetType: _w_AlgorithmSetType,
    AlgorithmTextType: _w_AlgorithmTextType,
    AliasSetType: _w_AliasSetType,
    AliasType: _w_AliasType,
    AncillaryDataSetType: _w_AncillaryDataSetType,
    AncillaryDataType: _w_AncillaryDataType,
    ArgumentANDedConditionsType: _w_ArgumentANDedConditionsType,
    ArgumentAbsoluteTimeDataType: _w_ArgumentAbsoluteTimeDataType,
    ArgumentArgumentRefEntryType: _w_ArgumentArgumentRefEntryType,
    ArgumentArrayArgumentRefEntryType: _w_ArgumentArrayArgumentRefEntryType,
    ArgumentArrayParameterRefEntryType: _w_ArgumentArrayParameterRefEntryType,
    ArgumentAssignmentListType: _w_ArgumentAssignmentListType,
    ArgumentAssignmentType: _w_ArgumentAssignmentType,
    ArgumentBaseDataType: _w_ArgumentBaseDataType,
    ArgumentBaseTimeDataType: _w_ArgumentBaseTimeDataType,
    ArgumentBinaryDataEncodingType: _w_ArgumentBinaryDataEncodingType,
    ArgumentBinaryDataType: _w_ArgumentBinaryDataType,
    ArgumentBooleanDataType: _w_ArgumentBooleanDataType,
    ArgumentBooleanExpressionType: _w_ArgumentBooleanExpressionType,
    ArgumentComparisonCheckType: _w_ArgumentComparisonCheckType,
    ArgumentComparisonListType: _w_ArgumentComparisonListType,
    ArgumentComparisonType: _w_ArgumentComparisonType,
    ArgumentContainerRefEntryType: _w_ArgumentContainerRefEntryType,
    ArgumentContainerSegmentRefEntryType:
            _w_ArgumentContainerSegmentRefEntryType,
    ArgumentDimensionListType: _w_ArgumentDimensionListType,
    ArgumentDimensionType: _w_ArgumentDimensionType,
    ArgumentDiscreteLookupListType: _w_ArgumentDiscreteLookupListType,
    ArgumentDiscreteLookupType: _w_ArgumentDiscreteLookupType,
    ArgumentDynamicValueType: _w_ArgumentDynamicValueType,
    ArgumentEnumeratedDataType: _w_ArgumentEnumeratedDataType,
    ArgumentFixedValueEntryType: _w_ArgumentFixedValueEntryType,
    ArgumentFloatDataType: _w_ArgumentFloatDataType,
    ArgumentIndirectParameterRefEntryType:
            _w_ArgumentIndirectParameterRefEntryType,
    ArgumentInputAlgorithmType: _w_ArgumentInputAlgorithmType,
    ArgumentInputSetType: _w_ArgumentInputSetType,
    ArgumentInstanceRefType: _w_ArgumentInstanceRefType,
    ArgumentIntegerDataType: _w_ArgumentIntegerDataType,
    ArgumentIntegerValueType: _w_ArgumentIntegerValueType,
    ArgumentListType: _w_ArgumentListType,
    ArgumentLocationInContainerInBitsType:
            _w_ArgumentLocationInContainerInBitsType,
    ArgumentMatchCriteriaType: _w_ArgumentMatchCriteriaType,
    ArgumentMathOperationType: _w_ArgumentMathOperationType,
    ArgumentORedConditionsType: _w_ArgumentORedConditionsType,
    ArgumentParameterRefEntryType: _w_ArgumentParameterRefEntryType,
    ArgumentParameterSegmentRefEntryType:
            _w_ArgumentParameterSegmentRefEntryType,
    ArgumentRelativeTimeDataType: _w_ArgumentRelativeTimeDataType,
    ArgumentRepeatType: _w_ArgumentRepeatType,
    ArgumentRestrictionListType: _w_ArgumentRestrictionListType,
    ArgumentSequenceEntryType: _w_ArgumentSequenceEntryType,
    ArgumentStreamSegmentEntryType: _w_ArgumentStreamSegmentEntryType,
    ArgumentStringDataEncodingType: _w_ArgumentStringDataEncodingType,
    ArgumentStringDataType: _w_ArgumentStringDataType,
    ArgumentType: _w_ArgumentType,
    ArgumentTypeSetType: _w_ArgumentTypeSetType,
    ArgumentVariableStringType: _w_ArgumentVariableStringType,
    ArrayArgumentType: _w_ArrayArgumentType,
    ArrayDataTypeType: _w_ArrayDataTypeType,
    ArrayParameterRefEntryType: _w_ArrayParameterRefEntryType,
    ArrayParameterType: _w_ArrayParameterType,
    AuthorSetType: _w_AuthorSetType,
    AutoInvertType: _w_AutoInvertType,
    BaseAlarmType: _w_BaseAlarmType,
    BaseCalibratorType: _w_BaseCalibratorType,
    BaseComparisonType: _w_BaseComparisonType,
    BaseConditionsType: _w_BaseConditionsType,
    BaseContainerType: _w_BaseContainerType,
    BaseDataType: _w_BaseDataType,
    BaseMetaCommandType: _w_BaseMetaCommandType,
    BaseTimeDataType: _w_BaseTimeDataType,
    BaseTriggerType: _w_BaseTriggerType,
    BinaryAlarmType: _w_BinaryAlarmType,
    BinaryArgumentType: _w_BinaryArgumentType,
    BinaryContextAlarmListType: _w_BinaryContextAlarmListType,
    BinaryContextAlarmType: _w_BinaryContextAlarmType,
    BinaryDataEncodingType: _w_BinaryDataEncodingType,
    BinaryDataType: _w_BinaryDataType,
    BinaryParameterType: _w_BinaryParameterType,
    BlockMetaCommandType: _w_BlockMetaCommandType,
    BooleanAlarmType: _w_BooleanAlarmType,
    BooleanArgumentType: _w_BooleanArgumentType,
    BooleanContextAlarmListType: _w_BooleanContextAlarmListType,
    BooleanContextAlarmType: _w_BooleanContextAlarmType,
    BooleanDataType: _w_BooleanDataType,
    BooleanExpressionType: _w_BooleanExpressionType,
    BooleanParameterType: _w_BooleanParameterType,
    ByteType: _w_ByteType,
    CRCType: _w_CRCType,
    CalibratorType: _w_CalibratorType,
    ChangeAlarmRangesType: _w_ChangeAlarmRangesType,
    ChangeValueType: _w_ChangeValueType,
    CheckWindowAlgorithmsType: _w_CheckWindowAlgorithmsType,
    CheckWindowType: _w_CheckWindowType,
    ChecksumType: _w_ChecksumType,
    CommandContainerEntryListType: _w_CommandContainerEntryListType,
    CommandContainerSetType: _w_CommandContainerSetType,
    CommandContainerType: _w_CommandContainerType,
    CommandMetaDataType: _w_CommandMetaDataType,
    CommandVerifierType: _w_CommandVerifierType,
    ComparisonCheckType: _w_ComparisonCheckType,
    ComparisonListType: _w_ComparisonListType,
    ComparisonType: _w_ComparisonType,
    CompleteVerifierType: _w_CompleteVerifierType,
    ConstantType: _w_ConstantType,
    ContainerBinaryDataEncodingType: _w_ContainerBinaryDataEncodingType,
    ContainerRefEntryType: _w_ContainerRefEntryType,
    ContainerRefSetType: _w_ContainerRefSetType,
    ContainerRefType: _w_ContainerRefType,
    ContainerSegmentRefEntryType: _w_ContainerSegmentRefEntryType,
    ContainerSetType: _w_ContainerSetType,
    ContainerType: _w_ContainerType,
    ContextCalibratorListType: _w_ContextCalibratorListType,
    ContextCalibratorType: _w_ContextCalibratorType,
    ContextMatchType: _w_ContextMatchType,
    ContextSignificanceListType: _w_ContextSignificanceListType,
    ContextSignificanceType: _w_ContextSignificanceType,
    CustomAlarmType: _w_CustomAlarmType,
    CustomStreamType: _w_CustomStreamType,
    DataEncodingType: _w_DataEncodingType,
    DescriptionType: _w_DescriptionType,
    DimensionListType: _w_DimensionListType,
    DimensionType: _w_DimensionType,
    DiscreteLookupListType: _w_DiscreteLookupListType,
    DiscreteLookupType: _w_DiscreteLookupType,
    DynamicValueType: _w_DynamicValueType,
    EncodingType: _w_EncodingType,
    EntryListType: _w_EntryListType,
    EnumeratedArgumentType: _w_EnumeratedArgumentType,
    EnumeratedDataType: _w_EnumeratedDataType,
    EnumeratedParameterType: _w_EnumeratedParameterType,
    EnumerationAlarmLevelType: _w_EnumerationAlarmLevelType,
    EnumerationAlarmListType: _w_EnumerationAlarmListType,
    EnumerationAlarmType: _w_EnumerationAlarmType,
    EnumerationContextAlarmListType: _w_EnumerationContextAlarmListType,
    EnumerationContextAlarmType: _w_EnumerationContextAlarmType,
    EnumerationListType: _w_EnumerationListType,
    ErrorDetectCorrectType: _w_ErrorDetectCorrectType,
    ExecutionVerifierType: _w_ExecutionVerifierType,
    ExternalAlgorithmSetType: _w_ExternalAlgorithmSetType,
    ExternalAlgorithmType: _w_ExternalAlgorithmType,
    FailedVerifierType: _w_FailedVerifierType,
    FixedFrameStreamType: _w_FixedFrameStreamType,
    FixedFrameSyncStrategyType: _w_FixedFrameSyncStrategyType,
    FlagType: _w_FlagType,
    FloatArgumentType: _w_FloatArgumentType,
    FloatDataEncodingType: _w_FloatDataEncodingType,
    FloatDataType: _w_FloatDataType,
    FloatDataValidRangeType: _w_FloatDataValidRangeType,
    FloatParameterType: _w_FloatParameterType,
    FloatRangeType: _w_FloatRangeType,
    FrameStreamType: _w_FrameStreamType,
    HeaderType: _w_HeaderType,
    HistorySetType: _w_HistorySetType,
    IndirectParameterRefEntryType: _w_IndirectParameterRefEntryType,
    InputAlgorithmType: _w_InputAlgorithmType,
    InputOutputAlgorithmType: _w_InputOutputAlgorithmType,
    InputOutputTriggerAlgorithmType: _w_InputOutputTriggerAlgorithmType,
    InputParameterInstanceRefType: _w_InputParameterInstanceRefType,
    InputSetType: _w_InputSetType,
    IntegerArgumentType: _w_IntegerArgumentType,
    IntegerDataEncodingType: _w_IntegerDataEncodingType,
    IntegerDataType: _w_IntegerDataType,
    IntegerDataValidRangeType: _w_IntegerDataValidRangeType,
    IntegerParameterType: _w_IntegerParameterType,
    IntegerRangeType: _w_IntegerRangeType,
    IntegerValueType: _w_IntegerValueType,
    InterlockType: _w_InterlockType,
    LeadingSizeType: _w_LeadingSizeType,
    LinearAdjustmentType: _w_LinearAdjustmentType,
    LocationInContainerInBitsType: _w_LocationInContainerInBitsType,
    MatchCriteriaType: _w_MatchCriteriaType,
    MathAlgorithmType: _w_MathAlgorithmType,
    MathOperationCalibratorType: _w_MathOperationCalibratorType,
    MathOperationType: _w_MathOperationType,
    MemberListType: _w_MemberListType,
    MemberType: _w_MemberType,
    MessageRefSetType: _w_MessageRefSetType,
    MessageRefType: _w_MessageRefType,
    MessageSetType: _w_MessageSetType,
    MessageType: _w_MessageType,
    MetaCommandSetType: _w_MetaCommandSetType,
    MetaCommandStepListType: _w_MetaCommandStepListType,
    MetaCommandStepType: _w_MetaCommandStepType,
    MetaCommandType: _w_MetaCommandType,
    MultiRangeType: _w_MultiRangeType,
    NameDescriptionType: _w_NameDescriptionType,
    NoteSetType: _w_NoteSetType,
    NumberFormatType: _w_NumberFormatType,
    NumericAlarmType: _w_NumericAlarmType,
    NumericContextAlarmListType: _w_NumericContextAlarmListType,
    NumericContextAlarmType: _w_NumericContextAlarmType,
    ORedConditionsType: _w_ORedConditionsType,
    OnContainerUpdateTriggerType: _w_OnContainerUpdateTriggerType,
    OnParameterUpdateTriggerType: _w_OnParameterUpdateTriggerType,
    OnPeriodicRateTriggerType: _w_OnPeriodicRateTriggerType,
    OptionalNameDescriptionType: _w_OptionalNameDescriptionType,
    OutputParameterRefType: _w_OutputParameterRefType,
    OutputSetType: _w_OutputSetType,
    PCMStreamType: _w_PCMStreamType,
    ParameterInstanceRefType: _w_ParameterInstanceRefType,
    ParameterPropertiesType: _w_ParameterPropertiesType,
    ParameterRefEntryType: _w_ParameterRefEntryType,
    ParameterRefType: _w_ParameterRefType,
    ParameterSegmentRefEntryType: _w_ParameterSegmentRefEntryType,
    ParameterSetType: _w_ParameterSetType,
    ParameterToSetListType: _w_ParameterToSetListType,
    ParameterToSetType: _w_ParameterToSetType,
    ParameterToSuspendAlarmsOnType: _w_ParameterToSuspendAlarmsOnType,
    ParameterType: _w_ParameterType,
    ParameterTypeSetType: _w_ParameterTypeSetType,
    ParameterValueChangeType: _w_ParameterValueChangeType,
    ParametersToSuspendAlarmsOnSetType: _w_ParametersToSuspendAlarmsOnSetType,
    ParityType: _w_ParityType,
    PercentCompleteType: _w_PercentCompleteType,
    PhysicalAddressSetType: _w_PhysicalAddressSetType,
    PhysicalAddressType: _w_PhysicalAddressType,
    PolynomialCalibratorType: _w_PolynomialCalibratorType,
    QueuedVerifierType: _w_QueuedVerifierType,
    RateInStreamSetType: _w_RateInStreamSetType,
    RateInStreamType: _w_RateInStreamType,
    RateInStreamWithStreamNameType: _w_RateInStreamWithStreamNameType,
    ReceivedVerifierType: _w_ReceivedVerifierType,
    ReferenceTimeType: _w_ReferenceTimeType,
    RelativeTimeArgumentType: _w_RelativeTimeArgumentType,
    RelativeTimeDataType: _w_RelativeTimeDataType,
    RelativeTimeParameterType: _w_RelativeTimeParameterType,
    RepeatType: _w_RepeatType,
    RestrictionCriteriaType: _w_RestrictionCriteriaType,
    SentFromRangeVerifierType: _w_SentFromRangeVerifierType,
    SequenceContainerType: _w_SequenceContainerType,
    SequenceEntryType: _w_SequenceEntryType,
    ServiceRefType: _w_ServiceRefType,
    ServiceSetType: _w_ServiceSetType,
    ServiceType: _w_ServiceType,
    SignificanceType: _w_SignificanceType,
    SimpleAlgorithmType: _w_SimpleAlgorithmType,
    SizeInBitsFixedType: _w_SizeInBitsFixedType,
    SizeInBitsType: _w_SizeInBitsType,
    SpaceSystemType: _w_SpaceSystemType,
    SplineCalibratorType: _w_SplineCalibratorType,
    SplinePointType: _w_SplinePointType,
    StreamRefType: _w_StreamRefType,
    StreamSegmentEntryType: _w_StreamSegmentEntryType,
    StreamSetType: _w_StreamSetType,
    StringAlarmLevelType: _w_StringAlarmLevelType,
    StringAlarmListType: _w_StringAlarmListType,
    StringAlarmType: _w_StringAlarmType,
    StringArgumentType: _w_StringArgumentType,
    StringContextAlarmListType: _w_StringContextAlarmListType,
    StringContextAlarmType: _w_StringContextAlarmType,
    StringDataEncodingType: _w_StringDataEncodingType,
    StringDataType: _w_StringDataType,
    StringParameterType: _w_StringParameterType,
    SyncPatternType: _w_SyncPatternType,
    SyncStrategyType: _w_SyncStrategyType,
    TelemetryMetaDataType: _w_TelemetryMetaDataType,
    TermType: _w_TermType,
    TimeAlarmRangesType: _w_TimeAlarmRangesType,
    TimeAlarmType: _w_TimeAlarmType,
    TimeAssociationType: _w_TimeAssociationType,
    TimeContextAlarmListType: _w_TimeContextAlarmListType,
    TimeContextAlarmType: _w_TimeContextAlarmType,
    ToStringType: _w_ToStringType,
    TransferredToRangeVerifierType: _w_TransferredToRangeVerifierType,
    TransmissionConstraintListType: _w_TransmissionConstraintListType,
    TransmissionConstraintType: _w_TransmissionConstraintType,
    TriggerSetType: _w_TriggerSetType,
    TriggeredMathOperationType: _w_TriggeredMathOperationType,
    UnitSetType: _w_UnitSetType,
    UnitType: _w_UnitType,
    ValidFloatRangeSetType: _w_ValidFloatRangeSetType,
    ValidIntegerRangeSetType: _w_ValidIntegerRangeSetType,
    ValueEnumerationType: _w_ValueEnumerationType,
    VariableFrameStreamType: _w_VariableFrameStreamType,
    VariableFrameSyncStrategyType: _w_VariableFrameSyncStrategyType,
    VariableStringType: _w_VariableStringType,
    VerifierSetType: _w_VerifierSetType,
    XORType: _w_XORType,
}

__all__ = [
    "NAMESPACE", "SCHEMA_VERSION", "ROOT_ELEMENT", "READERS", "WRITERS",
    "BasisType", "BitOrderType", "ByteOrderCommonType", "ChangeBasisType",
    "ChangeSpanType", "CharacterWidthType", "ChecksumnameType",
    "ComparisonOperatorsType", "ConcernLevelsType", "ConsequenceLevelType",
    "EpochTimeEnumsType", "FlagBitType", "FloatEncodingSizeInBitsType",
    "FloatEncodingType", "FloatSizeInBitsType", "FloatingPointNotationType",
    "IntegerEncodingType", "MathOperatorsType", "PCMType", "ParityFormType",
    "RadixType", "RangeFormType", "ReferenceLocationType", "ReferencePointType",
    "StringEncodingType", "SystemTypeType", "TelemetryDataSourceType",
    "TimeAssociationUnitType", "TimeUnitsType", "TimeWindowIsRelativeToType",
    "UnitFormType", "ValidationStatusType", "VerifierEnumerationType",
    "ANDedConditionsType", "AbsoluteTimeArgumentType", "AbsoluteTimeDataType",
    "AbsoluteTimeParameterType", "AcceptedVerifierType",
    "AggregateArgumentType", "AggregateDataType", "AggregateParameterType",
    "AlarmConditionsType", "AlarmMultiRangesType", "AlarmRangesType",
    "AlarmType", "AlgorithmSetType", "AlgorithmTextType", "AliasSetType",
    "AliasType", "AncillaryDataSetType", "AncillaryDataType",
    "ArgumentANDedConditionsType", "ArgumentAbsoluteTimeDataType",
    "ArgumentArgumentRefEntryType", "ArgumentArrayArgumentRefEntryType",
    "ArgumentArrayParameterRefEntryType", "ArgumentAssignmentListType",
    "ArgumentAssignmentType", "ArgumentBaseDataType",
    "ArgumentBaseTimeDataType", "ArgumentBinaryDataEncodingType",
    "ArgumentBinaryDataType", "ArgumentBooleanDataType",
    "ArgumentBooleanExpressionType", "ArgumentComparisonCheckType",
    "ArgumentComparisonListType", "ArgumentComparisonType",
    "ArgumentContainerRefEntryType", "ArgumentContainerSegmentRefEntryType",
    "ArgumentDimensionListType", "ArgumentDimensionType",
    "ArgumentDiscreteLookupListType", "ArgumentDiscreteLookupType",
    "ArgumentDynamicValueType", "ArgumentEnumeratedDataType",
    "ArgumentFixedValueEntryType", "ArgumentFloatDataType",
    "ArgumentIndirectParameterRefEntryType", "ArgumentInputAlgorithmType",
    "ArgumentInputSetType", "ArgumentInstanceRefType",
    "ArgumentIntegerDataType", "ArgumentIntegerValueType", "ArgumentListType",
    "ArgumentLocationInContainerInBitsType", "ArgumentMatchCriteriaType",
    "ArgumentMathOperationType", "ArgumentORedConditionsType",
    "ArgumentParameterRefEntryType", "ArgumentParameterSegmentRefEntryType",
    "ArgumentRelativeTimeDataType", "ArgumentRepeatType",
    "ArgumentRestrictionListType", "ArgumentSequenceEntryType",
    "ArgumentStreamSegmentEntryType", "ArgumentStringDataEncodingType",
    "ArgumentStringDataType", "ArgumentType", "ArgumentTypeSetType",
    "ArgumentVariableStringType", "ArrayArgumentType", "ArrayDataTypeType",
    "ArrayParameterRefEntryType", "ArrayParameterType", "AuthorSetType",
    "AutoInvertType", "BaseAlarmType", "BaseCalibratorType",
    "BaseComparisonType", "BaseConditionsType", "BaseContainerType",
    "BaseDataType", "BaseMetaCommandType", "BaseTimeDataType",
    "BaseTriggerType", "BinaryAlarmType", "BinaryArgumentType",
    "BinaryContextAlarmListType", "BinaryContextAlarmType",
    "BinaryDataEncodingType", "BinaryDataType", "BinaryParameterType",
    "BlockMetaCommandType", "BooleanAlarmType", "BooleanArgumentType",
    "BooleanContextAlarmListType", "BooleanContextAlarmType", "BooleanDataType",
    "BooleanExpressionType", "BooleanParameterType", "ByteType", "CRCType",
    "CalibratorType", "ChangeAlarmRangesType", "ChangeValueType",
    "CheckWindowAlgorithmsType", "CheckWindowType", "ChecksumType",
    "CommandContainerEntryListType", "CommandContainerSetType",
    "CommandContainerType", "CommandMetaDataType", "CommandVerifierType",
    "ComparisonCheckType", "ComparisonListType", "ComparisonType",
    "CompleteVerifierType", "ConstantType", "ContainerBinaryDataEncodingType",
    "ContainerRefEntryType", "ContainerRefSetType", "ContainerRefType",
    "ContainerSegmentRefEntryType", "ContainerSetType", "ContainerType",
    "ContextCalibratorListType", "ContextCalibratorType", "ContextMatchType",
    "ContextSignificanceListType", "ContextSignificanceType", "CustomAlarmType",
    "CustomStreamType", "DataEncodingType", "DescriptionType",
    "DimensionListType", "DimensionType", "DiscreteLookupListType",
    "DiscreteLookupType", "DynamicValueType", "EncodingType", "EntryListType",
    "EnumeratedArgumentType", "EnumeratedDataType", "EnumeratedParameterType",
    "EnumerationAlarmLevelType", "EnumerationAlarmListType",
    "EnumerationAlarmType", "EnumerationContextAlarmListType",
    "EnumerationContextAlarmType", "EnumerationListType",
    "ErrorDetectCorrectType", "ExecutionVerifierType",
    "ExternalAlgorithmSetType", "ExternalAlgorithmType", "FailedVerifierType",
    "FixedFrameStreamType", "FixedFrameSyncStrategyType", "FlagType",
    "FloatArgumentType", "FloatDataEncodingType", "FloatDataType",
    "FloatDataValidRangeType", "FloatParameterType", "FloatRangeType",
    "FrameStreamType", "HeaderType", "HistorySetType",
    "IndirectParameterRefEntryType", "InputAlgorithmType",
    "InputOutputAlgorithmType", "InputOutputTriggerAlgorithmType",
    "InputParameterInstanceRefType", "InputSetType", "IntegerArgumentType",
    "IntegerDataEncodingType", "IntegerDataType", "IntegerDataValidRangeType",
    "IntegerParameterType", "IntegerRangeType", "IntegerValueType",
    "InterlockType", "LeadingSizeType", "LinearAdjustmentType",
    "LocationInContainerInBitsType", "MatchCriteriaType", "MathAlgorithmType",
    "MathOperationCalibratorType", "MathOperationType", "MemberListType",
    "MemberType", "MessageRefSetType", "MessageRefType", "MessageSetType",
    "MessageType", "MetaCommandSetType", "MetaCommandStepListType",
    "MetaCommandStepType", "MetaCommandType", "MultiRangeType",
    "NameDescriptionType", "NoteSetType", "NumberFormatType",
    "NumericAlarmType", "NumericContextAlarmListType",
    "NumericContextAlarmType", "ORedConditionsType",
    "OnContainerUpdateTriggerType", "OnParameterUpdateTriggerType",
    "OnPeriodicRateTriggerType", "OptionalNameDescriptionType",
    "OutputParameterRefType", "OutputSetType", "PCMStreamType",
    "ParameterInstanceRefType", "ParameterPropertiesType",
    "ParameterRefEntryType", "ParameterRefType", "ParameterSegmentRefEntryType",
    "ParameterSetType", "ParameterToSetListType", "ParameterToSetType",
    "ParameterToSuspendAlarmsOnType", "ParameterType", "ParameterTypeSetType",
    "ParameterValueChangeType", "ParametersToSuspendAlarmsOnSetType",
    "ParityType", "PercentCompleteType", "PhysicalAddressSetType",
    "PhysicalAddressType", "PolynomialCalibratorType", "QueuedVerifierType",
    "RateInStreamSetType", "RateInStreamType", "RateInStreamWithStreamNameType",
    "ReceivedVerifierType", "ReferenceTimeType", "RelativeTimeArgumentType",
    "RelativeTimeDataType", "RelativeTimeParameterType", "RepeatType",
    "RestrictionCriteriaType", "SentFromRangeVerifierType",
    "SequenceContainerType", "SequenceEntryType", "ServiceRefType",
    "ServiceSetType", "ServiceType", "SignificanceType", "SimpleAlgorithmType",
    "SizeInBitsFixedType", "SizeInBitsType", "SpaceSystemType",
    "SplineCalibratorType", "SplinePointType", "StreamRefType",
    "StreamSegmentEntryType", "StreamSetType", "StringAlarmLevelType",
    "StringAlarmListType", "StringAlarmType", "StringArgumentType",
    "StringContextAlarmListType", "StringContextAlarmType",
    "StringDataEncodingType", "StringDataType", "StringParameterType",
    "SyncPatternType", "SyncStrategyType", "TelemetryMetaDataType", "TermType",
    "TimeAlarmRangesType", "TimeAlarmType", "TimeAssociationType",
    "TimeContextAlarmListType", "TimeContextAlarmType", "ToStringType",
    "TransferredToRangeVerifierType", "TransmissionConstraintListType",
    "TransmissionConstraintType", "TriggerSetType",
    "TriggeredMathOperationType", "UnitSetType", "UnitType",
    "ValidFloatRangeSetType", "ValidIntegerRangeSetType",
    "ValueEnumerationType", "VariableFrameStreamType",
    "VariableFrameSyncStrategyType", "VariableStringType", "VerifierSetType",
    "XORType", "ANDedConditionsChoice", "AcceptedVerifierChoice1",
    "AcceptedVerifierChoice2", "AlarmChoice", "AlgorithmSetChoice",
    "ArgumentANDedConditionsChoice", "ArgumentBaseDataChoice",
    "ArgumentBinaryDataChoice", "ArgumentBooleanDataChoice",
    "ArgumentBooleanExpressionChoice", "ArgumentComparisonCheckChoice1",
    "ArgumentComparisonCheckChoice2", "ArgumentComparisonChoice",
    "ArgumentDiscreteLookupChoice", "ArgumentDynamicValueChoice",
    "ArgumentEnumeratedDataChoice", "ArgumentFloatDataChoice",
    "ArgumentInputSetChoice", "ArgumentIntegerDataChoice",
    "ArgumentIntegerValueChoice", "ArgumentLocationInContainerInBitsChoice",
    "ArgumentMatchCriteriaChoice", "ArgumentMathOperationChoice",
    "ArgumentORedConditionsChoice", "ArgumentStringDataChoice",
    "ArgumentStringDataEncodingChoice", "ArgumentTypeSetChoice",
    "ArgumentVariableStringChoice1", "ArgumentVariableStringChoice2",
    "BaseDataChoice", "BinaryAlarmChoice", "BinaryArgumentChoice",
    "BinaryContextAlarmChoice", "BinaryDataChoice", "BinaryParameterChoice",
    "BooleanAlarmChoice", "BooleanArgumentChoice", "BooleanContextAlarmChoice",
    "BooleanDataChoice", "BooleanExpressionChoice", "BooleanParameterChoice",
    "CalibratorChoice", "CommandContainerEntryListChoice",
    "CommandVerifierChoice1", "CommandVerifierChoice2", "ComparisonCheckChoice",
    "CompleteVerifierChoice1", "CompleteVerifierChoice2", "ContextMatchChoice",
    "DiscreteLookupChoice", "EncodingChoice", "EntryListChoice",
    "EnumeratedArgumentChoice", "EnumeratedDataChoice",
    "EnumeratedParameterChoice", "EnumerationAlarmChoice",
    "EnumerationContextAlarmChoice", "ErrorDetectCorrectChoice",
    "ExecutionVerifierChoice1", "ExecutionVerifierChoice2",
    "FailedVerifierChoice1", "FailedVerifierChoice2", "FixedFrameStreamChoice",
    "FloatArgumentChoice", "FloatDataChoice", "FloatParameterChoice",
    "FrameStreamChoice", "InputSetChoice", "IntegerArgumentChoice",
    "IntegerDataChoice", "IntegerParameterChoice", "IntegerValueChoice",
    "LocationInContainerInBitsChoice", "MatchCriteriaChoice",
    "MathOperationCalibratorChoice", "MathOperationChoice",
    "MetaCommandSetChoice", "NumericAlarmChoice", "NumericContextAlarmChoice",
    "ORedConditionsChoice", "ParameterSetChoice", "ParameterToSetChoice",
    "ParameterTypeSetChoice", "PercentCompleteChoice", "QueuedVerifierChoice1",
    "QueuedVerifierChoice2", "ReceivedVerifierChoice1",
    "ReceivedVerifierChoice2", "ReferenceTimeChoice",
    "RestrictionCriteriaChoice", "SentFromRangeVerifierChoice1",
    "SentFromRangeVerifierChoice2", "ServiceChoice", "StreamSetChoice",
    "StringAlarmChoice", "StringArgumentChoice", "StringContextAlarmChoice",
    "StringDataChoice", "StringDataEncodingChoice", "StringParameterChoice",
    "TimeAlarmChoice", "TimeContextAlarmChoice",
    "TransferredToRangeVerifierChoice1", "TransferredToRangeVerifierChoice2",
    "TransmissionConstraintChoice", "TriggerSetChoice",
    "TriggeredMathOperationChoice", "VariableFrameStreamChoice",
    "VariableStringChoice1", "VariableStringChoice2"]
