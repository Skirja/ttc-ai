# Konvensi test TTC

- Integration test executable berada langsung di `tests/*.rs` dan menguji
  perilaku publik binary.
- Fixture immutable berada di `tests/fixtures/<family>/<case>/`. Setiap family
  filter nantinya menyediakan kasus success, failure, warning, unknown/edge,
  dan large sesuai milestone aktif.
- `javascript-real/` berisi proyek JS/TS dengan versi dependency terkunci;
  smoke test menyalinnya ke direktori sementara sebelum menjalankan tool.
- Data yang berubah selama test dibuat di temporary directory unik per test dan
  dibersihkan setelah test selesai. Test tidak boleh menulis ke working fixture.
- Test yang membaca HOME atau XDG wajib mengganti `HOME`, `XDG_CONFIG_HOME`,
  `XDG_DATA_HOME`, dan `XDG_STATE_HOME` pada child process dengan direktori
  temporary milik test. Jangan membaca atau menulis state pengguna asli.
- Fixture tidak boleh memuat credential, token, session, atau output autentikasi.
