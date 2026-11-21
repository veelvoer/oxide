function fish_prompt
    set -l last_status $status

    # Colours come from Oxide's theme: the file is rewritten when the theme changes, so shells that are open follow it
    set -l colors "@COLORS@"
    test -r $colors; and source $colors
    set -l bg_pill     (set -q oxide_pill;  and echo $oxide_pill;  or echo 343f44)
    set -l bg_git      (set -q oxide_git;   and echo $oxide_git;   or echo 4f5b58)
    set -l light_text  (set -q oxide_fg;    and echo $oxide_fg;    or echo d3c6aa)
    set -l bright_aqua (set -q oxide_aqua;  and echo $oxide_aqua;  or echo 83c092)
    set -l green_glow  (set -q oxide_green; and echo $oxide_green; or echo a7c080)
    set -l warm_amber  (set -q oxide_amber; and echo $oxide_amber; or echo dbbc7f)
    set -l error_red   (set -q oxide_red;   and echo $oxide_red;   or echo e67e80)

    echo -e "" # Spacer line

    # 1. Directory Pill [  ~/path ]
    set_color normal
    set_color $bg_pill
    echo -n ""
    set_color -b $bg_pill
    set_color --bold $bright_aqua
    echo -n "  " 
    set_color -b $bg_pill
    set_color $light_text
    echo -n (prompt_pwd)
    set_color normal
    set_color $bg_pill
    echo -n " "

    # 2. Git Pill [  main ]
    if type -q git; and command git rev-parse --is-inside-work-tree >/dev/null 2>&1
        set -l branch (command git branch --show-current 2>/dev/null)
        test -z "$branch"; and set branch (command git rev-parse --short HEAD 2>/dev/null)

        set_color normal
        set_color $bg_git
        echo -n ""
        set_color -b $bg_git
        set_color --bold $green_glow
        echo -n "  " 
        set_color -b $bg_git
        set_color $light_text
        echo -n "$branch"

        if command git status --porcelain 2>/dev/null | string length -q
            set_color -b $bg_git
            set_color --bold $error_red
            echo -n " •"
        end

        set_color normal
        set_color $bg_git
        echo -n " "
    end

    # 3. Clock Pill [  14:30 ]
    set_color normal
    set_color $bg_pill
    echo -n ""
    set_color -b $bg_pill
    set_color --bold $warm_amber
    echo -n "  " 
    set_color -b $bg_pill
    set_color $light_text
    echo -n (date "+%H:%M")
    set_color normal
    set_color $bg_pill
    echo -n ""

    # LINE 2: PROMPT SYMBOL
    echo ""
    if test $last_status -eq 0
        set_color --bold $green_glow
        echo -n " ❯ "
    else
        set_color --bold $error_red
        echo -n " ✖ "
    end

    set_color normal
end

function fish_right_prompt
end
