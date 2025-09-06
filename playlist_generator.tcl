#!/usr/bin/env tclsh

proc parse_music_input {input_text} {
    set lines [split [string trim $input_text] "\n"]
    
    set tracks {}
    set albums {}
    
    foreach line $lines {
        if {[string match "💿*" $line]} {
            # Remove 💿 and clean up - this is an album
            regsub {^💿\s*} $line {} content
            set content [string trim $content]
            lappend albums "\"$content\""
        } elseif {[string match "🎶*" $line]} {
            # Remove 🎶 and clean up - this is a track
            regsub {^🎶\s*} $line {} content
            set content [string trim $content]
            lappend tracks "\"$content\""
        }
    }
    
    return [list "" $tracks $albums]
}

proc create_playlist_and_generate_commands {playlist_name tracks albums} {
    set today [clock format [clock seconds] -format "%Y-%m-%d"]
    set safe_playlist_name "test-afera-$today"
    
    # Execute playlist creation command
    set create_cmd "target/release/spotify_player playlist new \"$safe_playlist_name\""
    puts "Executing: $create_cmd"
    
    if {[catch {exec {*}[split $create_cmd]} result]} {
        puts "Error creating playlist: $result"
        return
    }
    
    puts $result
    
    # Extract playlist ID from output
    if {[regexp {'spotify:playlist:([^']+)'} $result -> playlist_id]} {
        puts ""
        puts "# Using playlist ID: $playlist_id"
        puts ""
        
        # Generate album commands first (💿 lines)
        if {[llength $albums] > 0} {
            foreach album $albums {
                puts "target/release/spotify_player playlist edit --playlist-id \"$playlist_id\" --album-name $album add"
            }
        }
        
        # Generate track commands (🎶 lines)
        if {[llength $tracks] > 0} {
            foreach track $tracks {
                puts "target/release/spotify_player playlist edit --playlist-id \"$playlist_id\" --track-name $track add"
            }
        }
    } else {
        puts "Could not extract playlist ID from output: $result"
    }
}

proc main {} {
    global argc argv
    
    if {$argc > 0} {
        # Read from command line argument
        set input_text [lindex $argv 0]
    } else {
        # Read from stdin
        set input_text [read stdin]
    }
    
    lassign [parse_music_input $input_text] playlist_name tracks albums
    create_playlist_and_generate_commands $playlist_name $tracks $albums
}

main