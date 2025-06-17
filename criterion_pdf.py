#!/usr/bin/env python3
# playwright_criterion_pdf.py
# pip install playwright
# playwright install chromium

import os
import glob
from pathlib import Path
from playwright.sync_api import sync_playwright

def convert_criterion_reports():
    criterion_dir = Path("target/criterion")
    output_dir = Path("benchmark_pdfs")
    
    # Create output directory
    output_dir.mkdir(exist_ok=True)
    
    if not criterion_dir.exists():
        print("Error: Criterion directory not found. Run benchmarks first:")
        print("cargo bench --bench order_book_enhanced")
        return
    
    # Find all HTML files
    html_files = list(criterion_dir.glob("**/*.html"))
    
    if not html_files:
        print("No HTML files found in criterion directory")
        return
    
    print(f"Found {len(html_files)} HTML files to convert")
    
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page()
        
        for i, html_file in enumerate(html_files, 1):
            # Create a clean filename
            relative_path = html_file.relative_to(criterion_dir)
            pdf_name = str(relative_path).replace("/", "_").replace("\\", "_").replace(".html", ".pdf")
            pdf_path = output_dir / pdf_name
            
            print(f"Converting {i}/{len(html_files)}: {html_file} -> {pdf_path}")
            
            try:
                # Convert file path to file:// URL
                file_url = html_file.resolve().as_uri()
                page.goto(file_url, wait_until="networkidle")
                
                # Generate PDF
                page.pdf(
                    path=str(pdf_path),
                    format="A4",
                    margin={
                        "top": "0.75in",
                        "right": "0.75in", 
                        "bottom": "0.75in",
                        "left": "0.75in"
                    },
                    print_background=True
                )
                
            except Exception as e:
                print(f"  Warning: Failed to convert {html_file}: {e}")
        
        browser.close()
    
    print(f"\nAll reports converted to {output_dir}/")
    
    # Try to merge PDFs and clean up if successful
    merge_success = try_merge_pdfs(output_dir)
    
    if merge_success:
        cleanup_intermediate_pdfs(output_dir)

def try_merge_pdfs(output_dir):
    """Attempt to merge PDFs using available tools. Returns True if successful."""
    pdf_files = list(output_dir.glob("*.pdf"))
    
    if len(pdf_files) <= 1:
        print("Only one or no PDF files found, skipping merge")
        return False
    
    # Try PyPDF2
    try:
        from PyPDF2 import PdfMerger
        
        print(f"Merging {len(pdf_files)} PDFs...")
        merger = PdfMerger()
        
        # Sort files for consistent ordering
        pdf_files.sort()
        
        for pdf_file in pdf_files:
            print(f"  Adding {pdf_file.name}...")
            merger.append(str(pdf_file))
        
        merged_path = output_dir / "comprehensive_benchmark_report.pdf"
        with open(merged_path, 'wb') as output_file:
            merger.write(output_file)
        
        merger.close()
        print(f"Comprehensive report created: {merged_path}")
        return True
        
    except ImportError:
        print("Install PyPDF2 to merge PDFs: pip install PyPDF2")
        return False
    except Exception as e:
        print(f"Error merging PDFs: {e}")
        return False

def cleanup_intermediate_pdfs(output_dir):
    """Delete intermediate PDF files, keeping only the merged comprehensive report."""
    merged_file = output_dir / "comprehensive_benchmark_report.pdf"
    
    if not merged_file.exists():
        print("Comprehensive report not found, skipping cleanup")
        return
    
    # Find all PDF files except the merged one
    all_pdfs = list(output_dir.glob("*.pdf"))
    intermediate_pdfs = [pdf for pdf in all_pdfs if pdf.name != "comprehensive_benchmark_report.pdf"]
    
    if not intermediate_pdfs:
        print("No intermediate PDFs found to clean up")
        return
    
    print(f"\nCleaning up {len(intermediate_pdfs)} intermediate PDF files...")
    
    deleted_count = 0
    for pdf_file in intermediate_pdfs:
        try:
            pdf_file.unlink()
            print(f"  Deleted: {pdf_file.name}")
            deleted_count += 1
        except Exception as e:
            print(f"  Warning: Failed to delete {pdf_file.name}: {e}")
    
    print(f"Cleanup complete: {deleted_count} files deleted")
    print(f"Final report available at: {merged_file}")

if __name__ == "__main__":
    convert_criterion_reports()