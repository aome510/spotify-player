#!/usr/bin/env tclsh

package require http
package require tls

# Configure TLS for HTTPS requests
http::register https 443 ::tls::socket

proc scrape_afera_playlists {url} {
    # Fetch the webpage
    set token [http::geturl $url -timeout 10000]
    set status [http::status $token]
    set ncode [http::ncode $token]
    
    if {$status ne "ok" || $ncode != 200} {
        puts stderr "Error fetching webpage: $status (HTTP $ncode)"
        return
    }
    
    set html [http::data $token]
    http::cleanup $token
    
    # Extract albums and tracks using CSS class selectors
    extract_albums $html
    extract_tracks $html
}

proc extract_albums {html} {
    # Look for <p class='bold-yellow pull-left'> with PŁYTA TYGODNIA
    # and extract artist - album from <span> inside
    set pattern {<p class='bold-yellow pull-left'>\s*PŁYTA TYGODNIA\s*<span>([^<]+)</span>}
    
    set matches [regexp -all -inline $pattern $html]
    
    for {set i 0} {$i < [llength $matches]} {incr i 2} {
        set album_info [lindex $matches [expr $i + 1]]
        set album_info [string trim $album_info]
        
        # Clean up any HTML entities
        set album_info [regsub -all {&quot;} $album_info "\""]
        set album_info [regsub -all {&amp;} $album_info "&"]
        set album_info [regsub -all {&#039;} $album_info "'"]
        
        puts "💿 $album_info"
    }
}

proc extract_tracks {html} {
    # Look for <p class='bold-title-music'> and extract artist - track
    set pattern {<p class='bold-title-music'>([^<]+)</p>}
    
    set matches [regexp -all -inline $pattern $html]
    
    for {set i 0} {$i < [llength $matches]} {incr i 2} {
        set track_info [lindex $matches [expr $i + 1]]
        set track_info [string trim $track_info]
        
        # Clean up any HTML entities
        set track_info [regsub -all {&quot;} $track_info "\""]
        set track_info [regsub -all {&amp;} $track_info "&"]
        set track_info [regsub -all {&#039;} $track_info "'"]
        
        puts "🎶 $track_info"
    }
}

# Main execution
if {$argc < 1} {
    puts "Usage: $argv0 <url>"
    puts "Example: $argv0 https://www.afera.com.pl/muzyka"
    exit 1
}

set url [lindex $argv 0]

# Run the scraper
scrape_afera_playlists $url