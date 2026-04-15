#!/usr/bin/env python3
"""Create test fixtures for superbook-pdf TDD tests."""

import os
import sys
import struct
import zlib

def create_minimal_pdf(filename, pages=1, with_images=False):
    """Create a minimal valid PDF file."""
    
    # PDF header
    content = b"%PDF-1.4\n"
    
    # Objects
    objects = []
    obj_num = 1
    
    # Catalog
    catalog_num = obj_num
    objects.append(f"{obj_num} 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n")
    obj_num += 1
    
    # Pages object  
    pages_num = obj_num
    page_refs = " ".join([f"{3 + i} 0 R" for i in range(pages)])
    objects.append(f"{obj_num} 0 obj\n<< /Type /Pages /Kids [ {page_refs} ] /Count {pages} >>\nendobj\n")
    obj_num += 1
    
    # Page objects
    for i in range(pages):
        page_num = obj_num
        # A4 size: 595 x 842 points
        objects.append(f"{obj_num} 0 obj\n<< /Type /Page /Parent {pages_num} 0 R /MediaBox [ 0 0 595 842 ] /Contents {obj_num + 1} 0 R >>\nendobj\n")
        obj_num += 1
        
        # Page content stream
        stream_content = f"BT /F1 24 Tf 100 700 Td (Page {i + 1}) Tj ET".encode()
        objects.append(f"{obj_num} 0 obj\n<< /Length {len(stream_content)} >>\nstream\n".encode() + stream_content + b"\nendstream\nendobj\n")
        obj_num += 1
    
    # Build PDF content
    pdf_content = content
    xref_positions = []
    
    for obj in objects:
        xref_positions.append(len(pdf_content))
        if isinstance(obj, bytes):
            pdf_content += obj
        else:
            pdf_content += obj.encode()
    
    # Cross-reference table
    xref_start = len(pdf_content)
    xref = f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n"
    for pos in xref_positions:
        xref += f"{pos:010d} 00000 n \n"
    
    pdf_content += xref.encode()
    
    # Trailer
    trailer = f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref_start}\n%%EOF\n"
    pdf_content += trailer.encode()
    
    with open(filename, 'wb') as f:
        f.write(pdf_content)
    print(f"Created: {filename} ({pages} pages)")

def create_png_image(filename, width=100, height=100, color=(255, 255, 255)):
    """Create a minimal PNG image."""
    
    def png_chunk(chunk_type, data):
        chunk = chunk_type + data
        crc = zlib.crc32(chunk) & 0xffffffff
        return struct.pack('>I', len(data)) + chunk + struct.pack('>I', crc)
    
    # PNG signature
    signature = b'\x89PNG\r\n\x1a\n'
    
    # IHDR chunk
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)  # 8-bit RGB
    ihdr = png_chunk(b'IHDR', ihdr_data)
    
    # IDAT chunk (image data)
    raw_data = b''
    for y in range(height):
        raw_data += b'\x00'  # filter byte
        for x in range(width):
            raw_data += bytes(color)
    
    compressed = zlib.compress(raw_data)
    idat = png_chunk(b'IDAT', compressed)
    
    # IEND chunk
    iend = png_chunk(b'IEND', b'')
    
    with open(filename, 'wb') as f:
        f.write(signature + ihdr + idat + iend)
    print(f"Created: {filename} ({width}x{height})")

def create_grayscale_png(filename, width=100, height=100, gray=128):
    """Create a grayscale PNG image."""
    
    def png_chunk(chunk_type, data):
        chunk = chunk_type + data
        crc = zlib.crc32(chunk) & 0xffffffff
        return struct.pack('>I', len(data)) + chunk + struct.pack('>I', crc)
    
    signature = b'\x89PNG\r\n\x1a\n'
    
    # IHDR - grayscale (color type 0)
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 0, 0, 0, 0)
    ihdr = png_chunk(b'IHDR', ihdr_data)
    
    raw_data = b''
    for y in range(height):
        raw_data += b'\x00'
        for x in range(width):
            raw_data += bytes([gray])
    
    compressed = zlib.compress(raw_data)
    idat = png_chunk(b'IDAT', compressed)
    iend = png_chunk(b'IEND', b'')
    
    with open(filename, 'wb') as f:
        f.write(signature + ihdr + idat + iend)
    print(f"Created: {filename} (grayscale {width}x{height})")

def create_test_image_with_content(filename, width=200, height=300):
    """Create a test image with some content (black text area on white)."""
    
    def png_chunk(chunk_type, data):
        chunk = chunk_type + data
        crc = zlib.crc32(chunk) & 0xffffffff
        return struct.pack('>I', len(data)) + chunk + struct.pack('>I', crc)
    
    signature = b'\x89PNG\r\n\x1a\n'
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    ihdr = png_chunk(b'IHDR', ihdr_data)
    
    raw_data = b''
    for y in range(height):
        raw_data += b'\x00'
        for x in range(width):
            # Create a border and some content area
            if x < 20 or x >= width - 20 or y < 20 or y >= height - 20:
                # White border/margin
                raw_data += bytes([255, 255, 255])
            elif 50 < y < 100 and 30 < x < width - 30:
                # Dark content area (simulating text)
                raw_data += bytes([50, 50, 50])
            else:
                # White background
                raw_data += bytes([255, 255, 255])
    
    compressed = zlib.compress(raw_data)
    idat = png_chunk(b'IDAT', compressed)
    iend = png_chunk(b'IEND', b'')
    
    with open(filename, 'wb') as f:
        f.write(signature + ihdr + idat + iend)
    print(f"Created: {filename} (content image {width}x{height})")

def create_skewed_image(filename, width=200, height=300, skew_degrees=2):
    """Create an image that appears skewed (diagonal lines)."""
    import math
    
    def png_chunk(chunk_type, data):
        chunk = chunk_type + data
        crc = zlib.crc32(chunk) & 0xffffffff
        return struct.pack('>I', len(data)) + chunk + struct.pack('>I', crc)
    
    signature = b'\x89PNG\r\n\x1a\n'
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 2, 0, 0, 0)
    ihdr = png_chunk(b'IHDR', ihdr_data)
    
    raw_data = b''
    skew_tan = math.tan(math.radians(skew_degrees))
    
    for y in range(height):
        raw_data += b'\x00'
        for x in range(width):
            # Create diagonal lines to simulate skew
            adjusted_x = x - int(y * skew_tan)
            if 40 <= adjusted_x <= 160 and (y % 20 < 3):
                raw_data += bytes([0, 0, 0])  # Black line
            else:
                raw_data += bytes([255, 255, 255])  # White
    
    compressed = zlib.compress(raw_data)
    idat = png_chunk(b'IDAT', compressed)
    iend = png_chunk(b'IEND', b'')
    
    with open(filename, 'wb') as f:
        f.write(signature + ihdr + idat + iend)
    print(f"Created: {filename} (skewed image {skew_degrees}°)")

def main():
    fixtures_dir = os.path.dirname(os.path.abspath(__file__))
    os.chdir(fixtures_dir)
    
    # Create PDF fixtures
    create_minimal_pdf("sample.pdf", pages=1)
    create_minimal_pdf("10pages.pdf", pages=10)
    create_minimal_pdf("a4.pdf", pages=1)
    create_minimal_pdf("multipage.pdf", pages=5)
    create_minimal_pdf("color.pdf", pages=1)
    
    # Create image fixtures
    create_png_image("white.png", 100, 100, (255, 255, 255))
    create_png_image("black.png", 100, 100, (0, 0, 0))
    create_png_image("sample_page.png", 595, 842, (255, 255, 255))  # A4 at 72 DPI
    create_grayscale_png("grayscale.png", 100, 100, 128)
    
    # Test images with content
    create_test_image_with_content("page_with_content.png", 200, 300)
    create_test_image_with_content("book_page_1.png", 400, 600)
    create_test_image_with_content("page_with_number_42.png", 400, 600)
    create_test_image_with_content("page_no_number.png", 400, 600)
    
    # Skewed images for deskew tests
    create_skewed_image("skewed_2deg.png", 200, 300, 2)
    create_skewed_image("skewed_5deg.png", 200, 300, 5)
    create_skewed_image("slightly_skewed.png", 200, 300, 1)
    create_skewed_image("heavily_skewed.png", 200, 300, 10)
    
    # Book page images for batch tests
    for i in range(1, 11):
        create_test_image_with_content(f"book_page_{i}.png", 400, 600)
    
    print(f"\nAll fixtures created in {fixtures_dir}")

if __name__ == "__main__":
    main()
