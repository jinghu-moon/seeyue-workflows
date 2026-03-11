#!/usr/bin/env node
"use strict";

const { runHookAndExit } = require("../runtime/hook-client.cjs");

runHookAndExit("PostToolUse:Bash");
