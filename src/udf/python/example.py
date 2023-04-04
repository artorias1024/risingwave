import socket
from typing import Iterator, List, Optional, Tuple
from risingwave.udf import udf, udtf, UdfServer
import random
import struct
from decimal import Decimal


@udf(input_types=[], result_type='INT')
def random_int() -> int:
    return random.randint(0, 100)


@udf(input_types=['INT', 'INT'], result_type='INT')
def gcd(x: int, y: int) -> int:
    while y != 0:
        (x, y) = (y, x % y)
    return x


@udf(name='gcd3', input_types=['INT', 'INT', 'INT'], result_type='INT')
def gcd3(x: int, y: int, z: int) -> int:
    return gcd(gcd(x, y), z)


@udtf(input_types='INT', result_types='INT')
def series(n: int) -> Iterator[int]:
    for i in range(n):
        yield i


@udtf(input_types=['INT'], result_types=['INT', 'VARCHAR'])
def series2(n: int) -> Iterator[Tuple[int, str]]:
    for i in range(n):
        yield i, str(i)


@udf(input_types=['BYTEA'], result_type='STRUCT<VARCHAR, VARCHAR, SMALLINT, SMALLINT>')
def extract_tcp_info(tcp_packet: bytes):
    src_addr, dst_addr = struct.unpack('!4s4s', tcp_packet[12:20])
    src_port, dst_port = struct.unpack('!HH', tcp_packet[20:24])
    src_addr = socket.inet_ntoa(src_addr)
    dst_addr = socket.inet_ntoa(dst_addr)
    return src_addr, dst_addr, src_port, dst_port


@udf(input_types='VARCHAR', result_type='DECIMAL')
def hex_to_dec(hex: Optional[str]) -> Optional[Decimal]:
    if not hex:
        return None

    hex = hex.strip()
    dec = Decimal(0)

    while hex:
        chunk = hex[:16]
        chunk_value = int(hex[:16], 16)
        dec = dec * (1 << (4 * len(chunk))) + chunk_value
        hex = hex[16:]
    return dec


@udf(input_types=["VARCHAR[]", "INT"], result_type="VARCHAR")
def array_access(list: List[str], idx: int) -> Optional[str]:
    if idx == 0 or idx > len(list):
        return None
    return list[idx - 1]


if __name__ == '__main__':
    server = UdfServer(location="0.0.0.0:8815")
    server.add_function(random_int)
    server.add_function(gcd)
    server.add_function(gcd3)
    server.add_function(series)
    server.add_function(series2)
    server.add_function(extract_tcp_info)
    server.add_function(hex_to_dec)
    server.add_function(array_access)
    server.serve()
