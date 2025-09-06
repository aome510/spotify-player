#!/bin/bash

# Local test script for Afera playlist automation
# This script simulates the GitHub Actions workflow locally

set -e

echo "🧪 Testing Afera Playlist Automation Locally"
echo "=============================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if scripts exist
echo -e "${YELLOW}Checking required files...${NC}"
if [[ ! -f "playlist_scraper.tcl" ]]; then
    echo -e "${RED}❌ playlist_scraper.tcl not found${NC}"
    exit 1
fi

if [[ ! -f "playlist_generator.tcl" ]]; then
    echo -e "${RED}❌ playlist_generator.tcl not found${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Required scripts found${NC}"

# Check if TCL is installed
echo -e "${YELLOW}Checking TCL installation...${NC}"
if ! command -v tclsh &> /dev/null; then
    echo -e "${RED}❌ TCL is not installed. Install with: brew install tcl-tk (macOS) or apt-get install tcl (Ubuntu)${NC}"
    exit 1
fi

# Check TCL TLS package
echo -e "${YELLOW}Checking TCL TLS package...${NC}"
if ! echo 'package require tls; puts "TLS OK"' | tclsh 2>/dev/null; then
    echo -e "${RED}❌ TCL TLS package not found. Install with: brew install tcl-tk (macOS) or apt-get install tcl-tls (Ubuntu)${NC}"
    exit 1
fi

echo -e "${GREEN}✅ TCL and TLS package available${NC}"

# Make scripts executable
echo -e "${YELLOW}Making scripts executable...${NC}"
chmod +x playlist_scraper.tcl
chmod +x playlist_generator.tcl
echo -e "${GREEN}✅ Scripts are executable${NC}"

# Test scraper
echo -e "${YELLOW}Testing playlist scraper...${NC}"
echo "Scraping Afera website (this may take a few seconds)..."

if ./playlist_scraper.tcl https://www.afera.com.pl/muzyka > test_scraped_content.txt 2>/dev/null; then
    echo -e "${GREEN}✅ Scraper completed successfully${NC}"
    
    # Show scraped content
    echo -e "${YELLOW}Scraped content:${NC}"
    cat test_scraped_content.txt
    
    # Count items
    albums=$(grep -c "^💿" test_scraped_content.txt || true)
    tracks=$(grep -c "^🎶" test_scraped_content.txt || true)
    
    echo ""
    echo -e "${GREEN}📊 Found: $albums albums, $tracks tracks${NC}"
    
    if [[ $albums -eq 0 && $tracks -eq 0 ]]; then
        echo -e "${RED}❌ No content found - check website structure${NC}"
        exit 1
    fi
else
    echo -e "${RED}❌ Scraper failed${NC}"
    exit 1
fi

# Test playlist generator (dry run)
echo -e "${YELLOW}Testing playlist generator...${NC}"

# Check if spotify_player binary exists
if [[ ! -f "target/release/spotify_player" ]]; then
    echo -e "${YELLOW}⚠️  spotify_player binary not found. Building...${NC}"
    if command -v cargo &> /dev/null; then
        echo "Building spotify_player..."
        if cargo build --release --no-default-features --features "rodio-backend,media-control,image,notify" 2>/dev/null; then
            echo -e "${GREEN}✅ Build successful${NC}"
        else
            echo -e "${YELLOW}⚠️  Build failed, but will test generator logic anyway${NC}"
        fi
    else
        echo -e "${YELLOW}⚠️  Cargo not found, skipping build${NC}"
    fi
fi

# Test generator with scraped content
echo "Testing playlist generator with scraped content..."
if cat test_scraped_content.txt | ./playlist_generator.tcl > test_playlist_commands.txt 2>/dev/null; then
    echo -e "${GREEN}✅ Generator completed successfully${NC}"
    
    echo -e "${YELLOW}Generated commands:${NC}"
    head -20 test_playlist_commands.txt
    
    # Count generated commands
    command_count=$(grep -c "target/release/spotify_player" test_playlist_commands.txt || true)
    echo ""
    echo -e "${GREEN}📊 Generated $command_count spotify_player commands${NC}"
else
    echo -e "${RED}❌ Generator failed${NC}"
    exit 1
fi

# Test complete pipeline
echo -e "${YELLOW}Testing complete pipeline...${NC}"
if ./playlist_scraper.tcl https://www.afera.com.pl/muzyka | ./playlist_generator.tcl > test_complete_pipeline.txt 2>/dev/null; then
    echo -e "${GREEN}✅ Complete pipeline test successful${NC}"
    
    # Show summary
    lines=$(wc -l < test_complete_pipeline.txt)
    echo -e "${GREEN}📊 Pipeline generated $lines lines of output${NC}"
else
    echo -e "${RED}❌ Complete pipeline test failed${NC}"
    exit 1
fi

# Cleanup
echo -e "${YELLOW}Cleaning up test files...${NC}"
rm -f test_scraped_content.txt test_playlist_commands.txt test_complete_pipeline.txt
echo -e "${GREEN}✅ Cleanup complete${NC}"

echo ""
echo -e "${GREEN}🎉 ALL TESTS PASSED!${NC}"
echo -e "${GREEN}The automation is ready for GitHub Actions deployment.${NC}"
echo ""
echo -e "${YELLOW}Next steps:${NC}"
echo "1. Set up GitHub secrets: SPOTIFY_CLIENT_ID, SPOTIFY_CLIENT_SECRET, SPOTIFY_REFRESH_TOKEN"
echo "2. Push the code to GitHub"
echo "3. The workflow will run every Monday at 10:00 AM UTC"
echo "4. You can also trigger it manually from GitHub Actions tab"