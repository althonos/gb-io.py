from datetime import date
from typing import Optional, Union, BinaryIO, List, Iterator, Iterable

__author__: str
__version__: str

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
    date: Optional[date]
    @property
    def sequence(self) -> bytes: ...
    @property
    def features(self) -> List[Feature]: ...
    @property
    def references(self) -> List[Reference]: ...

class Source:
    name: str
    organism: Optional[str]
    def __init__(self, name: str, organism: Optional[str]): ...
    def __repr__(self) -> str: ...

class Feature:
    kind: str
    @property
    def location(self) -> Location: ...
    @property
    def qualifiers(self) -> List[Qualifier]: ...

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
    def __init__(
        self, start: int, end: int, before: bool = False, after: bool = False
    ): ...
    def __repr__(self) -> str: ...

class Between(Location):
    start: int
    end: int
    def __init__(self, start: int, end: int): ...
    def __repr__(self) -> str: ...

class Complement(Location):
    @property
    def start(self) -> int: ...
    @property
    def end(self) -> int: ...
    def __init__(self, location: Location): ...
    def __repr__(self) -> str: ...

class Join(Location):
    @property
    def start(self) -> int: ...
    @property
    def end(self) -> int: ...
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class Order(Location):
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class Bond(Location):
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class OneOf(Location):
    def __init__(self, locations: List[Location]): ...
    def __repr__(self) -> str: ...

class External(Location):
    accession: str
    @property
    def location(self) -> Optional[Location]: ...
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
