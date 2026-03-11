#!/usr/bin/env node
        "use strict";

        const fs = require("node:fs");
        const os = require("node:os");
        const path = require("node:path");
        const { spawnSync } = require("node:child_process");

        const projectRoot = path.resolve(__dirname, "..", "..");
        const validatorPath = path.join(projectRoot, "scripts", "runtime", "validate-specs.cjs");
        const fixtureRoot = path.join(__dirname, "spec-fixtures");

        function copyDir(sourcePath, targetPath) {
          fs.mkdirSync(path.dirname(targetPath), { recursive: true });
          fs.cpSync(sourcePath, targetPath, { recursive: true });
        }

        function copySkills(targetRoot) {
          const source = path.join(projectRoot, ".agents", "skills");
          const target = path.join(targetRoot, ".agents", "skills");
          if (!fs.existsSync(source)) {
            return;
          }
          fs.mkdirSync(path.dirname(target), { recursive: true });
          fs.cpSync(source, target, { recursive: true });
        }

        function overlayDir(sourcePath, targetPath) {
          if (!fs.existsSync(sourcePath)) {
            return;
          }
          fs.cpSync(sourcePath, targetPath, { recursive: true, force: true });
        }

        function makeTempRoot() {
          return fs.mkdtempSync(path.join(os.tmpdir(), "sy-runtime-specs-"));
        }

        function runValidator(rootDir) {
          return spawnSync(process.execPath, [validatorPath, "--all", "--root", rootDir], {
            cwd: projectRoot,
            encoding: "utf8",
          });
        }

        function runManifestValidator(rootDir) {
          return spawnSync(process.execPath, [validatorPath, "--manifest", "--root", rootDir], {
            cwd: projectRoot,
            encoding: "utf8",
          });
        }

        function runGateValidator(rootDir, gate) {
          return spawnSync(process.execPath, [validatorPath, "--all", "--gate", gate, "--root", rootDir], {
            cwd: projectRoot,
            encoding: "utf8",
          });
        }

        const cases = {
          "valid-root": {
            execute() {
              return runValidator(projectRoot);
            },
            expectStatus: 0,
            expectSignals: ["SPEC_VALIDATION_PASS"],
          },
          "manifest-only": {
            execute() {
              return runManifestValidator(projectRoot);
            },
            expectStatus: 0,
            expectSignals: ["SPEC_VALIDATION_PASS"],
          },
          "invalid-cross-ref": {
            execute() {
              const tempRoot = makeTempRoot();
              copyDir(path.join(projectRoot, "workflow"), path.join(tempRoot, "workflow"));
              copySkills(tempRoot);
              overlayDir(path.join(fixtureRoot, "invalid-cross-ref"), tempRoot);
              return runValidator(tempRoot);
            },
            expectStatus: 1,
            expectSignals: ["INVALID_CAPABILITY_REF", "SPEC_VALIDATION_FAIL"],
          },
          "invalid-file-class-ref": {
            execute() {
              const tempRoot = makeTempRoot();
              copyDir(path.join(projectRoot, "workflow"), path.join(tempRoot, "workflow"));
              copySkills(tempRoot);
              overlayDir(path.join(fixtureRoot, "invalid-file-class-ref"), tempRoot);
              return runValidator(tempRoot);
            },
            expectStatus: 1,
            expectSignals: ["INVALID_FILE_CLASS_REF", "SPEC_VALIDATION_FAIL"],
          },
          "invalid-command-class-ref": {
            execute() {
              const tempRoot = makeTempRoot();
              copyDir(path.join(projectRoot, "workflow"), path.join(tempRoot, "workflow"));
              copySkills(tempRoot);
              overlayDir(path.join(fixtureRoot, "invalid-command-class-ref"), tempRoot);
              return runValidator(tempRoot);
            },
            expectStatus: 1,
            expectSignals: ["INVALID_COMMAND_CLASS_REF", "SPEC_VALIDATION_FAIL"],
          },
          "freeze-gate-blocks": {
            execute() {
              const tempRoot = makeTempRoot();
              copyDir(path.join(projectRoot, "workflow"), path.join(tempRoot, "workflow"));
              copySkills(tempRoot);
              return runGateValidator(tempRoot, "P2-N1");
            },
            expectStatus: 1,
            expectSignals: ["SPEC_NOT_FROZEN_AT_REQUIRED_GATE", "SPEC_VALIDATION_FAIL"],
          },
          "invalid-router-parallel-mode": {
            execute() {
              const tempRoot = makeTempRoot();
              copyDir(path.join(projectRoot, "workflow"), path.join(tempRoot, "workflow"));
              copySkills(tempRoot);
              const routerPath = path.join(tempRoot, "workflow", "router.spec.yaml");
              const mutated = fs
                .readFileSync(routerPath, "utf8")
                .replace("parallel_phases_supported: false", "parallel_phases_supported: true")
                .replace("parallel_nodes_supported: false", "parallel_nodes_supported: true");
              fs.writeFileSync(routerPath, mutated, "utf8");
              return runValidator(tempRoot);
            },
            expectStatus: 1,
            expectSignals: ["PARALLEL_PHASES_NOT_SUPPORTED_V1", "PARALLEL_NODES_NOT_SUPPORTED_V1", "SPEC_VALIDATION_FAIL"],
          },
        };

        function parseArgs(argv) {
          const parsed = { caseName: null };
          for (let index = 0; index < argv.length; index += 1) {
            if (argv[index] === "--case") {
              index += 1;
              parsed.caseName = argv[index];
              continue;
            }
            throw new Error(`Unknown argument: ${argv[index]}`);
          }
          return parsed;
        }

        function main() {
          let parsed;
          try {
            parsed = parseArgs(process.argv.slice(2));
          } catch (error) {
            console.error(`ARG_PARSE_FAIL ${error.message}`);
            process.exit(1);
          }

          const selectedEntries = parsed.caseName
            ? [[parsed.caseName, cases[parsed.caseName]]]
            : Object.entries(cases);

          if (selectedEntries.some(([, entry]) => !entry)) {
            console.error(`UNKNOWN_CASE ${parsed.caseName}`);
            process.exit(1);
          }

          let failed = false;

          for (const [caseName, entry] of selectedEntries) {
            const run = entry.execute();
            const combined = `${run.stdout || ""}
${run.stderr || ""}`;
            const hasStatus = run.status === entry.expectStatus;
            const hasSignals = entry.expectSignals.every((signal) => combined.includes(signal));
            if (!hasStatus || !hasSignals) {
              failed = true;
              console.error(`CASE_FAIL ${caseName}`);
              console.error(`Expected status: ${entry.expectStatus}; actual: ${run.status}`);
              console.error(combined.trim());
              continue;
            }
            console.log(`CASE_PASS ${caseName}`);
          }

          if (failed) {
            process.exit(1);
          }

          console.log("SPEC_FIXTURES_PASS");
        }

        main();
