# Matriks command M8

| Family pada SPEC §8 | Command yang diuji | Record yang dapat dikompaksi | Raw/retained wajib | Fixture dan smoke |
|---|---|---|---|---|
| Build tools | `cmake --build`, `ctest`, `ninja`, `make test`, `make check` | Persentase CMake, aksi compile/link `[N/M]`, baris CTest `Passed` dengan jumlah/durasi | Target CMake/Ninja/Make dinamis; CTest dashboard/XML; compile/build summary; test gagal, warning, diagnostic | `build_tool_fixtures`; `remaining_ecosystem_e2e`; `m8-smoke.sh` |
| Ruby | `rspec`, `bundle exec rspec`, `rubocop`, `rake test` | Baris yang seluruhnya berisi titik passing atau meter default | Formatter/report lain, offense, correction, failure, summary, output aplikasi | `ruby_swift_fixtures`; `remaining_ecosystem_e2e`; project Ruby terkunci |
| Swift | `swift build`, `swift test` | Baris compile/module/link serta XCTest/Swift Testing passing dengan durasi valid | Failure, warning, source location, summary; `--show-bin-path` dan JSON | `ruby_swift_fixtures`; `remaining_ecosystem_e2e`; Swift Package smoke |
| Docker/Compose | `docker build`, `docker compose build` | Metadata internal BuildKit untuk load definition, ignore file, context, atau metadata image | Langkah `RUN`/`COPY`, hasil aplikasi, image/build summary, JSON/rawjson, warning/error/security | `infrastructure_fixtures`; `remaining_ecosystem_e2e`; local Dockerfile dan Compose smoke |
| Podman | `podman build` | Hanya metadata internal BuildKit bila output tool cocok grammar yang sama | STEP/COPY/RUN, COMMIT, diagnostic, warning/error/security; format lain raw | `infrastructure_fixtures`; `remaining_ecosystem_e2e`; digest image Podman smoke |
| Terraform | `terraform validate` | Tidak ada; diagnostic-only | Semua diagnostic, validasi berhasil, JSON, plan/diff/resource change | `infrastructure_fixtures`; `remaining_ecosystem_e2e`; HCL valid/tidak valid |
| Helm | `helm lint` | Banner `==> Linting CHART` yang cocok grammar | Lint result, rekomendasi, diagnostic, failure, summary | `infrastructure_fixtures`; `remaining_ecosystem_e2e`; local chart smoke |
| Raw/interactive matrix | Reader, network/SSH, database client, unknown app, `git diff/show`, Compose up/run TTY, `kubectl logs --follow`, `kubectl exec -it` | Tidak ada | Byte stdout/stderr, status, satu invocation, dan streaming/input interaktif | `raw_command_matrix`; regresi process/TTY/signal tetap pada test M2 |

Setiap large fixture menambahkan summary/diagnostic yang wajib terlihat dan
mengukur byte TTC termasuk metadata. Pinned real-tool E2E membandingkan status
direct/TTC serta memeriksa diagnostic yang sama.
