---
title: Phase 3 — WIP January 2026 Restructure Analysis
date: 2026-04-20
status: analysis (pre-plan)
---

# Phase 3 — WIP `january-2026-restructure` 분석

Write-capability를 위해 보류된 WIP 브랜치(`origin/wip/january-2026-restructure`, +5223 / −1023 lines, 기반: `df5a53d` — Phase 1/2 이전)에서 Phase 3에 참고할 **좋은 패턴**과 **회피할 함정**을 추출한다.

Worktree: `/tmp/websh-wip/` (작업 완료 후 정리).

---

## 1. 그대로 가져올 가치가 있는 것 (as-is / minor edit)

### 1.1 `PendingChanges` + `StagedChanges` 두 레이어
- 파일: `src/core/storage/pending.rs:53-188`, `staged.rs:10-68`
- 설계: `PendingChanges`는 `HashMap<path, PendingChange>` + `Vec<path>`(삽입 순서). `ChangeType`에 `CreateFile / CreateBinaryFile / UpdateFile / DeleteFile / CreateDirectory / DeleteDirectory` 6변형. `StagedChanges`는 단순 `HashSet<path>` — **pending에서 선택된 경로만 가리키는 포인터** 역할.
- 장점: Git mental-model과 일치. Serialize/Clone 가능 → localStorage 왕복. 테스트 5개 포함.
- 적용: 그대로 포팅 가능. Phase 2 `AppError`에 `From<StorageError>` 추가만 필요.

### 1.2 `StorageBackend` trait (`BoxFuture`로 object-safe)
- 파일: `src/core/storage/backend.rs:17-60`
- 설계: `type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>` 로 trait object 안전. 메서드: `create/update/delete_file`, `create/delete_directory`, `commit(&PendingChanges) -> Manifest`, `get_file_sha`, `is_authenticated`, `backend_type`.
- 장점: 다중 백엔드(IPFS 등) 확장 경로 열어둠. WASM에서 `async fn in trait` 제약 우회.
- 적용: 그대로. 단 `commit` 반환값 `Manifest`는 크립토 결정 이후 shape 확정.

### 1.3 `GitHubBackend` 실구현 (⚠️ 서브에이전트 오류 정정)
- 파일: `src/core/storage/github.rs:1-511` (특히 `commit` at `451-493`)
- **서브에이전트는 "commit이 stub"이라 보고했으나 사실이 아님**. Backend 쪽 `commit`은 fully implemented: changes를 순회하며 각 CRUD를 `await`, 이어서 `update_manifest`로 manifest.json을 재직렬화·재업로드. `terminal.rs:107-150`에 `backend.commit(&pending, &msg).await` 실제 호출 경로가 존재.
- **진짜 stub은 CLI 핸들러** `execute.rs::execute_sync_commit` (`880-919`) — 동기 `execute_command`가 future를 반환할 수 없어 "Ready to commit... (Note: Full async commit implementation pending)" 메시지만 출력하고, 실제 async 작업은 UI 버튼(SyncPanel commit)에서 별도 진입. 이것은 bug가 아니라 **의도적 설계 결정** (async 작업은 CLI가 아닌 UI 핸들러에서).
- 적용: backend 자체는 그대로 포팅. CLI와 async의 연결 방식(아래 §3.2)은 Phase 1의 `SideEffect` 패러다임에 맞춰 재설계 필요.
- 에러 매핑: HTTP 401/403/404/409/422/429 → `StorageError::{NotAuthenticated, PermissionDenied, NotFound, Conflict, AlreadyExists, RateLimited}` 매핑 깔끔 (`github.rs:165-185`).

### 1.4 `admin.rs` — 권한 게이팅
- 파일: `src/core/admin.rs:1-128`
- 설계: `ADMIN_ADDRESSES: &[&str]` 상수 + `is_admin(&WalletState)` 대소문자 무시 비교. `AdminStatus` enum (`Admin / NotAdmin / Connecting / Disconnected`)으로 UI 디스플레이 지원.
- 장점: 간단하고 테스트 가능. 외부 네트워크 없음.
- 적용: 그대로 포팅. 단 향후 manifest/ENS 기반 동적 allowlist로의 업그레이드 경로 고민.

### 1.5 `MergedFs` 컴퓨티드 뷰 로직
- 파일: `src/core/filesystem/merged.rs:17-149` (특히 `list_dir`의 overlay 병합 `58-88`, `extract_child_name` `152-168`)
- 설계: `VirtualFs` (base) + `PendingChanges` (overlay)를 합쳐 `get_entry / list_dir / exists / is_directory / get_pending_content` 제공. 기존 `&VirtualFs` 호출부를 `&MergedFs`로 바꾸면 write 플로우가 투명하게 적용됨.
- 단위 테스트 6개 (`286-431`) 포함.
- 적용: 포팅 가치 높음. **단 state.rs와의 결합에 버그 있음 — §2.1 참조**.

### 1.6 Markdown 이미지 data-URL rewriting
- 파일: `src/utils/markdown.rs` (`markdown_to_html_with_images`)
- 설계: 프리뷰 시 `pending.get_all_binary_data_urls()`로 data URL 맵을 만들고, markdown 렌더링 후 `<img src>`를 치환. ammonia CSP `"data"` scheme 허용 (`markdown.rs:52`).
- 적용: 포팅. 단 binary storage 전략이 바뀌면 (§2.2) 입력 소스만 교체.

---

## 2. 심각한 문제 — 포팅 전 반드시 고쳐야 할 것

### 2.1 `FsState` 메모이제이션 부재 (문서와 구현 불일치)
- 파일: `src/core/filesystem/state.rs:40-42`, doc-comment at `merged.rs:12-16`
- 주장: "This is a computed snapshot managed by `FsState` with automatic memoization"
- 실제: `pub fn get(&self) -> MergedFs { MergedFs::new(self.base.get(), self.pending.get()) }` — 호출할 때마다 `VirtualFs` 전체를 clone. Memo 없음.
- 영향: 파일 1000개 이상 시 Reader/Explorer 리렌더마다 전체 FS 복제 → Phase 2 Track F의 "no double-clone" 원칙 정면 위배.
- 해결책: `base: RwSignal<Rc<VirtualFs>>`로 변경 (cheap clone). 또는 `Memo<MergedFs>`를 `FsState`에 내장해 `base/pending` 변경 시에만 재계산. 전자가 더 단순.

### 2.2 `CreateBinaryFile { content_base64: String }` → localStorage 할당 초과
- 파일: `src/core/storage/pending.rs:19-27`, 저장 호출 `reader/mod.rs:308-311`
- 문제: 이미지 5MB → base64 6.7MB. 모든 pending을 JSON으로 localStorage에 직렬화 → 브라우저 한도 5–10MB 급속 소진. 실패는 silent (`let _ = save_pending_changes(p);`).
- 해결책 후보:
  1. 크기 상한 (`MAX_PENDING_BINARY_BYTES`) + 거부 메시지.
  2. Binary는 IndexedDB로 분리 (pending 메타는 localStorage 유지).
  3. Binary는 memory-only (reload 시 손실, 경고 표시).
- 권고: 최소 (1) 즉시, (2)는 Phase 3 후반.

### 2.3 `reader/mod.rs`의 `console::log_1` 스팸 — 14회
- 파일: `src/components/reader/mod.rs:204-322` 구간에 14개
- save 핫패스(키스트로크)에 동기 console I/O. 디버깅 잔해.
- 해결책: 포팅 시 전부 삭제. 필요하면 `cfg(debug_assertions)` 또는 feature flag로 게이팅.

### 2.4 실패 silent swallow
- 파일: `reader/mod.rs:308-311` — `let _ = save_pending_changes(p);`
- `QuotaExceededError`가 그대로 사라짐. 유저는 저장 실패를 모름.
- 해결책: AppError로 래핑해 터미널 출력 / toast로 노출. 저장 실패 시 최소 이전 상태 유지 보장.

### 2.5 Contents API 순차 호출 — 원자성 없음
- 파일: `github.rs:459-486` (`for change in changes.iter() { ... await? }`)
- N개 파일 중 5번째에서 네트워크 오류 시: 4개는 commit됨, manifest는 아직 미반영 → 저장소 불일치 상태.
- 해결책: **Git Data API**로 교체 (create-blob × N → create-tree → create-commit → update-ref = 1 atomic ref 업데이트). 구현 비용 약간 높지만 정합성 훨씬 우수. 또는 최소한 실패 지점까지의 부분 커밋을 UI에 명시적으로 보고.

### 2.6 Rate-limit / 재시도 없음
- 파일: `github.rs:117-200` (`api_request`)
- 429 → `StorageError::RateLimited` 반환만, 백오프/재시도 없음.
- 단일 사용자 스케일에선 5000 req/hr 여유 있으나, Phase 3 후반에 `backon` 등으로 지수 백오프 추가.

### 2.7 커밋 중 race condition
- 파일: 여러 곳 (`sync_panel.rs`, `reader/mod.rs`, `terminal.rs:107-150`)
- 시나리오: commit 진행 중 사용자가 같은 파일 편집 → pending 갱신 → commit은 구 스냅샷 기반으로 진행 → 새 편집 사라지거나 덮어씌워짐.
- 해결책: `ctx.fs.is_committing: RwSignal<bool>` 도입. UI 편집 disable + CLI `touch/rm/sync add` 거부.

### 2.8 `sync discard` 확인 없음
- 파일: `execute.rs` 근처 `execute_sync_discard`
- 전체 pending 삭제가 확인 없이 즉시. 오타 한 번으로 작업 손실.
- 해결책: `sync discard --force` 요구, 없으면 "N changes will be lost. Use --force to confirm".

---

## 3. Phase 1/2와의 비호환 — 아키텍처 결정 필요

### 3.1 `CommandResult` shape 충돌
| | Phase 1/2 main | WIP |
|---|---|---|
| 반환 구조 | `{ output, exit_code, side_effect: Option<SideEffect> }` | `{ output, navigate_to, pending, staged }` |
| 철학 | Tag-enum 단일 효과, `dispatch_side_effect` 단일 퍼널 | 평면 다중 필드, 호출자가 각 필드 분기 처리 |
| Exit code | `i32` 있음 (POSIX) | **없음** |

- WIP의 평면 구조는 "`touch` 는 pending 갱신 + 옵션으로 navigate" 같은 다중 효과를 자연스럽게 표현.
- Phase 1의 tag enum은 단일 퍼널 유지, 테스트 용이성, 의도 명확성에서 유리.
- **Open decision**: 둘 다 장단. 가능한 합성:
  - `SideEffect` enum에 `UpdatePending(PendingChanges)`, `UpdateStaged(StagedChanges)`, `Commit { backend_tag, message }` 변형 추가.
  - `Vec<SideEffect>`로 다중 효과 허용 (tag 유지하면서 WIP의 additive성 확보).
  - `exit_code` 반드시 유지. 쓰기 명령 POSIX 의미: `touch` 기존=0, `rm` 미존재=1, `mkdir` 디렉토리 존재=1, `sync commit` staged-없음=2.
- **결정 전에 쓰기 명령(touch/mkdir/rm/sync \*) 실제 시나리오를 2-3개 구체화 → 시나리오별로 두 shape 모두 써보고 비교.**

### 3.2 Async dispatch 경계
- WIP는 "CLI 명령은 동기, async는 UI 버튼에서" 설계 (terminal.rs 두 경로 존재).
- 장점: 구조 단순. 단점: `sync commit` CLI의 UX가 어색 (실제 커밋 안 함).
- 후보:
  - **(A) WIP 유지**: CLI는 status/가이드만, 실 커밋은 SyncPanel 버튼. 분리 명확.
  - **(B) `SideEffect::Commit {...}`**: CLI `sync commit`이 이 variant 반환 → `dispatch_side_effect`에서 `spawn_local`로 async 분기. 단일 퍼널 유지.
- **Open decision**: (B)가 Phase 1 철학과 일치하고 CLI-first UX에 정합. 단 `dispatch_side_effect` 비동기화가 다른 variant에도 전염될 위험 (Login 등은 현재 동기로 충분). → `SideEffect`에 `Async(Box<dyn Future<...>>)` 또는 enum 변형별 분기 처리.

### 3.3 Phase 4 (크립토 결정)이 Phase 3에 선행
- ROADMAP §Phase 4가 명시. 쓰기가 추가되면:
  - Option A (실구현): 커밋 시 각 파일마다 AES key 생성 → 수신자별 `eth_getEncryptionPublicKey`로 wrap → `ChangeType::CreateFile.meta.encryption` 필드가 load-bearing.
  - Option B (리브랜드): 쓰기 경로 평문, `encryption` 필드 제거/명명 변경.
- WIP는 이를 전혀 고려하지 않음 (`meta.encryption`을 pass-through).
- **Phase 3 착수 전 Option A/B 결정 필수**. Option B가 훨씬 가볍고 로드맵 권고.

---

## 4. 부수적 관찰

### 4.1 `AppContext` 확장
- WIP `src/app.rs:196`: `fs: FsState` 필드 추가, `src/app.rs:216`: 초기화 시 `load_pending_changes()`로 localStorage 복원.
- Phase 1/2는 `ctx.fs: RwSignal<VirtualFs>` — Phase 3는 `ctx.fs: FsState { base, pending, staged }`로 업그레이드. 호출부(`ctx.fs.get()` → `ctx.fs.base()` 또는 `.with()`) 전반 수정 필요.

### 4.2 `MountRegistry::is_writable()` 확장 준비됨
- Phase 1 `MountRegistry`는 `writable` 필드 추가 위한 구조 이미 갖춤. WIP는 `models/mount.rs`에서 `Mount::writable: bool`까지 이미 도입 (`+233 -83` 라인 변경).
- 적용: Phase 3에서 mount별 writability 부여해 admin만 쓰기 허용/읽기 전용 mount 구분.

### 4.3 `Command` enum 확장 shape
- 파일: `src/core/commands/mod.rs:1-120` (diff)
- 추가 variant: `Touch(PathArg) / Mkdir(PathArg) / Rm(PathArg) / Rmdir(PathArg) / Sync(SyncSubcommand)`. `SyncSubcommand` enum에 `Status / Add / Reset / Commit / Discard / Auth`.
- 설계 좋음. parsing부터 단계적으로 포팅 가능.

---

## 5. Open decisions (Phase 3 계획 전에 잠금 필요)

1. **Phase 4 먼저 결정** — 크립토 Option A / B. 권고: B (리브랜드) → Phase 3 단순화.
2. **`CommandResult` shape** — 단일 tag `SideEffect` vs `Vec<SideEffect>` vs 평면 필드.
   - 시나리오 스케치 후 결정. 추천: `Vec<SideEffect>` + `exit_code` 유지.
3. **Async dispatch** — UI-only (WIP 방식) vs `SideEffect::Async*` (통합 퍼널).
   - 추천: 후자. 단일 `spawn_local` 진입점.
4. **Binary storage** — localStorage 제한 / IndexedDB / memory-only. 권고: 단기 상한 + 장기 IndexedDB.
5. **Commit atomicity** — Contents API (WIP, 순차 N-call, 비원자) vs Git Data API (1 ref 업데이트).
   - 추천: Git Data API. 초기 비용 있지만 정합성 크게 개선.
6. **`MergedFs` memoization** — `Rc<VirtualFs>` signal vs `Memo<MergedFs>` vs 호출부 `.with()` 리팩토링.
   - 추천: `Rc<VirtualFs>`. 가장 적은 변경면적.

---

## 6. 적용 순서 (예비 — Open decisions 해결 후 확정)

티어 1 (의존성 없음, 저리스크):
- `admin.rs` 그대로.
- `Command` enum 확장 + parser (테스트 포함).
- `AppError`에 `From<StorageError>` impl.

티어 2 (데이터 모델, 중립):
- `PendingChanges / StagedChanges / ChangeType` 포팅.
- `MergedFs` + `FsState` (메모이제이션 포함).
- `AppContext`에 `FsState` 연결, localStorage 복원.

티어 3 (백엔드, 결정 의존):
- `StorageBackend` trait.
- `GitHubBackend` — Contents API 또는 Git Data API 중 선택한 쪽.
- Async dispatch 전략 (§3.2 결정) 구현.

티어 4 (UI):
- Editor + Preview (console.log 제거).
- SyncPanel.
- `is_committing` flag로 UI 잠금.

티어 5 (하드닝):
- 쿼터 가드, 실패 toast.
- Rate-limit 재시도.
- `sync discard` confirm.

---

## 참조

- WIP worktree: `/tmp/websh-wip/` (필요 시 `git -C /home/wonjae/code/websh worktree remove /tmp/websh-wip`로 정리).
- 분기점: `df5a53d` (`git merge-base main origin/wip/january-2026-restructure`).
- 단일 커밋: `4fec9f4` "wip: january 2026 restructure (preserved for analysis)".
- 변경 규모: 48 files, +5223 / −1023.
