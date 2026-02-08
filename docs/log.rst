Logging
=======
Log files have three parts:

Header
    A description of the location of data in the file
    
Index
    This is a binary-search oriented index at the beginning of the
    file

Data
    Logged data.

Both parts are comprised of BLOCK_SIZE byte blocks.

Header
------
Number of index levels as a u64 value, followed by an offset from the
beginning of data for each index level.

Index
-----
Each index level is comprised of BLOCK_SIZE blocks. Within each block
are items comprised of two elements:

timestamp
    u64 time since the UNIX epoch in nanosecond resolution

data offset
    Number of bytes offset from the beginning of the file. If this is
    zero, instead of pointing to the beginning of the header, it indicates
    this entry is unused.

Data
----
Each block has a one-byte indicator with one of the following values:

BLOCK_START
    Data following the indicator is the beginning of a log record.

BLOCK_CONTINUE
    The block starts with data continued from the previous block.

Log Records
^^^^^^^^^^^
A log records consists of:

# A little-endian u64 length of the record

# A little-ending u64 timestamp in nanosecond units (not that the spacecraft
    system clock may not supply this level of resolution)

# The log record data.
