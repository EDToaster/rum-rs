#!/bin/bash

set -euo pipefail

rum spinner -s circle -t "Initializing quantum flux capacitors..." -- sleep 2 
rum spinner -s monkey -t "Warning: Don't set yourself on fire." -- sleep 1
rum spinner -s meter -t "Consuming all those consumables..." -- sleep 2

rum typer -t "Game is ready ..." -w 2000 -i 50

rum confirm -n "No thank you" -y "🪙" -t "Insert coin ..." || { rum typer -t "Sorry to see you go!" -w 2000 -i 50 ; exit ; }

USERNAME=$(rum text -p "What is your name?")

DIFF=$(printf "Easy\nMedium\nHard" | rum choose -t "Choose a difficulty, ${USERNAME}")

rum typer -t "Must've been a ${DIFF} decision." -w 1000 -i 50
rum typer -t "Well that's it!" -w 1000 -i 50
rum typer -t "Thanks for playing, ${USERNAME}." -w 1000 -i 50