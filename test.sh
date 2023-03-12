#!/bin/bash

set -euo pipefail

NAME=$(rum text -p "Enter name...")

RESPONSE=$(rum confirm -t "Hi, $NAME, are you ready?" && echo "Lets go!" || echo "Sorry, better luck next time")

rum spinner -s Arrow -t "Faffing about" -- sleep 2
rum spinner -s Moon -t "Generating crystals" -- sleep 2

echo "$RESPONSE"