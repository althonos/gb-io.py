import datetime
from typing import Optional, Union, BinaryIO, List, Iterator, Iterable

try:
    from typing import Literal
except ImportError:
    from typing_extensions import Literal  # type: ignore

__author__: str
__version__: str

_STRAND = Literal["+", "-"]

class Record:
    name: Optional[str]
    length: Optional[int]
    molecule_type: Optional[str]
    division: str
    definition: Optional[str]
    accession: Optional[str]
    version: Optional[str]
    dblink: Optional[str]
    keywords: Optional[str]
    circular: bool
    date: Optional[datetime.date]
    sequence: bytearray
    features: List[Feature]
    references: List[Reference]
    def __init__(
        self,
        sequence: Union[bytes, bytearray, memoryview],
        *,
        name: Optional[str] = None,
        length: Optional[str] = None,
        molecule_type: Optional[str] = None,
        division: str = "UNK",
        definition: Optional[str] = None,
        accession: Optional[str] = None,
        version: Optional[str] = None,
        dblink: Optional[str] = None,
        keywords: Optional[str] = None,
        circular: bool = False,
        date: Optional[datetime.date] = None,
        source: Optional[Source] = None,
        contig: Optional[Location] = None,
        references: Optional[Iterable[Reference]] = None,
        features: Optional[Iterable[Feature]] = None,
    ): ...

class Source:
    name: str
    organism: Optional[str]
    def __init__(self, name: str, organism: Optional[str]): ...
    def __repr__(self) -> str: ...

class Feature:
    kind: str
    location: Location
    qualifiers: List[Qualifier]
    def __init__(
        self, kind: str, location: Location, qualifiers: Optional[List[Qualifier]]
    ): ...
    def __repr__(self) -> str: ...

class Qualifier:
    key: str
    value: Optional[str]
    def __init__(self, key: str, value: Optional[str] = None): ...
    def __repr__(self) -> str: ...

class Location:
    pass

class Range(Location):
    start: int
    end: int
    before: bool
    after: bool
    @property
    def strand(self) -> _STRAND: ...
    def __init__(
        self, start: int, end: int, before: bool = False, after: bool = False
    ): ...
    def __repr__(self) -> str: ...

class Between(Location):
    start: int
    end: int
    @property
    def strand(self) -> _STRAND: ...
    def __init__(self, start: int, end: int): ...
    def __repr__(self) -> str: ...

class Complement(Location):
    location: Location
    @property
    def start(self) -> int: ...
    @property
    def end(self) -> int: ...
    @property
    def strand(self) -> _STRAND: ...
    def __init__(self, location: Location): ...
    def __repr__(self) -> str: ...

class Join(Location):
    locations: List[Location]
    @property
    def start(self) -> int: ...
    @property
    def end(self) -> int: ...
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class Order(Location):
    locations: List[Location]
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class Bond(Location):
    locations: List[Location]
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class OneOf(Location):
    locations: List[Location]
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class External(Location):
    accession: str
    location: Optional[Location]
    def __init__(self, accession: str, location: Optional[Location] = None): ...
    def __repr__(self) -> str: ...

class Reference:
    description: str
    title: str
    authors: Optional[str]
    consortium: Optional[str]
    journal: Optional[str]
    pubmed: Optional[str]
    remark: Optional[str]

def load(fh: Union[str, BinaryIO]) -> List[Record]: ...
def iter(fh: Union[str, BinaryIO]) -> Iterator[Record]: ...
def dump(
    records: Union[Record, Iterable[Record]],
    fh: Union[str, BinaryIO],
    escape_locus: bool = False,
    truncate_locus: bool = False,
): ...
