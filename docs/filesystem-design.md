# Virtual Filesystem Design

## 개요

웹 기반 터미널을 위한 가상 파일시스템(VFS) 설계.
Unix 파일시스템의 핵심 개념을 차용하되, 웹 환경과 암호학 기반 접근 제어에 맞게 단순화.

## 설계 원칙

- **암호학 기반 접근 제어**: Unix 권한(rwx) 대신 암호화로 접근 제어
- **다중 저장소 지원**: 마운트를 통해 GitHub, IPFS, IndexedDB 등 다양한 백엔드 통합
- **단일 트리**: 모든 마운트된 파일시스템이 하나의 `FsEntry` 트리로 통합
- **비동기 I/O**: 네트워크 저장소를 위한 async 연산

---

## 데이터 구조

### FsEntry (파일시스템 엔트리)

```rust
pub enum FsEntry {
    Directory {
        children: HashMap<String, FsEntry>,
        meta: FileMetadata,
    },
    File {
        content_path: Option<String>,  // 외부 저장소 경로
        description: String,
        meta: FileMetadata,
    },
}
```

### FileMetadata (메타데이터)

```rust
pub struct FileMetadata {
    // 기본 정보
    pub size: Option<u64>,          // 바이트 단위
    pub created: Option<u64>,       // Unix timestamp
    pub modified: Option<u64>,      // Unix timestamp

    // 암호화 정보
    pub encrypted: bool,
    pub encryption: Option<EncryptionInfo>,
}
```

### EncryptionInfo (암호화 정보)

대칭키로 파일을 암호화하고, 허용된 수신자들의 공개키로 대칭키를 래핑하는 방식.

```rust
pub struct EncryptionInfo {
    /// 사용된 암호화 알고리즘 (예: "AES-256-GCM")
    pub algorithm: String,

    /// 수신자별 래핑된 대칭키 목록
    pub wrapped_keys: Vec<WrappedKey>,
}

pub struct WrappedKey {
    /// 수신자 식별자 (지갑 주소 또는 공개키)
    pub recipient: String,

    /// 수신자의 공개키로 암호화된 대칭키
    pub encrypted_symmetric_key: Vec<u8>,
}
```

#### 암호화 플로우

```
암호화 (파일 저장 시):
┌─────────────────────────────────────────────────────────┐
│ 1. 랜덤 대칭키 생성 (AES-256)                           │
│ 2. 대칭키로 파일 내용 암호화                            │
│ 3. 각 수신자의 공개키로 대칭키 래핑                     │
│ 4. 암호화된 파일 + EncryptionInfo 저장                  │
└─────────────────────────────────────────────────────────┘

복호화 (파일 읽기 시):
┌─────────────────────────────────────────────────────────┐
│ 1. EncryptionInfo에서 내 공개키에 해당하는 WrappedKey 찾기 │
│ 2. 내 개인키로 대칭키 언래핑                            │
│ 3. 대칭키로 파일 내용 복호화                            │
└─────────────────────────────────────────────────────────┘
```

---

## VirtualFs (가상 파일시스템)

### 구조

```rust
pub struct VirtualFs {
    /// 파일시스템 루트 (모든 마운트 포함)
    root: FsEntry,

    /// 마운트 포인트별 백엔드 매핑
    mount_points: HashMap<String, MountInfo>,
}

pub struct MountInfo {
    /// 저장소 백엔드
    pub backend: Box<dyn StorageBackend>,

    /// 읽기 전용 여부
    pub readonly: bool,

    /// 마운트 시간
    pub mounted_at: u64,
}
```

### 트리 구조 예시

```
VirtualFs
├── root: FsEntry
│   ├── /
│   │   ├── home/
│   │   │   └── wonjae/          ← GitHubBackend에서 로드
│   │   │       ├── blog/
│   │   │       │   ├── hello.md
│   │   │       │   └── rust.md
│   │   │       ├── projects/
│   │   │       └── secrets/     ← 암호화된 파일들
│   │   │           └── keys.enc (encrypted: true)
│   │   ├── shared/              ← IpfsBackend에서 로드 (마운트)
│   │   │   └── docs/
│   │   └── tmp/                 ← IndexedDbBackend (로컬 임시)
│   │       └── draft.md
│
└── mount_points:
    ├── "/" → GitHubBackend
    ├── "/shared" → IpfsBackend
    └── "/tmp" → IndexedDbBackend
```

---

## StorageBackend (저장소 인터페이스)

### Trait 정의

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 백엔드 이름 (디버깅/로깅용)
    fn name(&self) -> &str;

    /// manifest 가져오기 (마운트 시 호출)
    async fn fetch_manifest(&self) -> Result<Vec<ManifestEntry>, StorageError>;

    /// 파일 내용 읽기
    async fn read(&self, path: &str) -> Result<Vec<u8>, StorageError>;

    /// 파일 쓰기 (새 경로/CID 반환)
    async fn write(&self, path: &str, data: &[u8], meta: &FileMetadata) -> Result<String, StorageError>;

    /// 파일/디렉토리 삭제
    async fn delete(&self, path: &str) -> Result<(), StorageError>;

    /// 파일/디렉토리 이동
    async fn rename(&self, from: &str, to: &str) -> Result<(), StorageError>;
}
```

### 구현체

| 백엔드 | 용도 | 읽기 | 쓰기 |
|--------|------|------|------|
| `GitHubBackend` | 메인 콘텐츠 저장소 | ✅ | ✅ (admin) |
| `IpfsBackend` | 탈중앙화 저장소 | ✅ | ✅ |
| `IndexedDbBackend` | 로컬 임시 저장소 | ✅ | ✅ |

---

## 연산

### 읽기 연산 (기존)

```rust
impl VirtualFs {
    /// 경로로 엔트리 조회
    pub fn get_entry(&self, path: &str) -> Option<&FsEntry>;

    /// 디렉토리 내용 나열
    pub fn list_dir(&self, path: &str) -> Option<Vec<DirEntry>>;

    /// 파일 내용 경로 조회
    pub fn get_file_content_path(&self, path: &str) -> Option<String>;

    /// 디렉토리 여부 확인
    pub fn is_directory(&self, path: &str) -> bool;
}
```

### 쓰기 연산 (추가)

```rust
impl VirtualFs {
    /// 파일 생성
    pub async fn create_file(&mut self, path: &str, content: &[u8]) -> Result<(), FsError>;

    /// 파일 덮어쓰기
    pub async fn write_file(&mut self, path: &str, content: &[u8]) -> Result<(), FsError>;

    /// 파일/디렉토리 삭제
    pub async fn delete(&mut self, path: &str) -> Result<(), FsError>;

    /// 이동/이름 변경
    pub async fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError>;

    /// 디렉토리 생성
    pub async fn mkdir(&mut self, path: &str) -> Result<(), FsError>;
}
```

### 암호화 연산 (추가)

```rust
impl VirtualFs {
    /// 파일 암호화 (수신자 목록 지정)
    pub async fn encrypt(&mut self, path: &str, recipients: &[String]) -> Result<(), FsError>;

    /// 파일 복호화 (현재 사용자의 키로)
    pub async fn decrypt(&self, path: &str, private_key: &[u8]) -> Result<Vec<u8>, FsError>;

    /// 암호화된 파일에 수신자 추가
    pub async fn grant_access(&mut self, path: &str, recipient: &str) -> Result<(), FsError>;

    /// 암호화된 파일에서 수신자 제거
    pub async fn revoke_access(&mut self, path: &str, recipient: &str) -> Result<(), FsError>;
}
```

### 마운트 연산 (추가)

```rust
impl VirtualFs {
    /// 백엔드를 특정 경로에 마운트
    pub async fn mount(&mut self, path: &str, backend: Box<dyn StorageBackend>) -> Result<(), FsError> {
        // 1. 백엔드에서 manifest 가져오기
        let entries = backend.fetch_manifest().await?;

        // 2. FsEntry 트리로 변환
        let subtree = Self::build_tree_from_manifest(&entries);

        // 3. root 트리의 path 위치에 삽입
        self.insert_at(path, subtree)?;

        // 4. mount_points에 기록
        self.mount_points.insert(path.to_string(), MountInfo {
            backend,
            readonly: false,
            mounted_at: current_timestamp(),
        });

        Ok(())
    }

    /// 마운트 해제
    pub fn unmount(&mut self, path: &str) -> Result<(), FsError>;

    /// 경로에 해당하는 백엔드 찾기
    fn get_backend_for_path(&self, path: &str) -> Option<&MountInfo> {
        // 가장 긴 매칭 마운트 포인트 반환
        self.mount_points
            .iter()
            .filter(|(mount_path, _)| path.starts_with(mount_path.as_str()))
            .max_by_key(|(mount_path, _)| mount_path.len())
            .map(|(_, info)| info)
    }
}
```

---

## 에러 타입

```rust
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("Path not found: {0}")]
    NotFound(String),

    #[error("Not a directory: {0}")]
    NotADirectory(String),

    #[error("Not a file: {0}")]
    NotAFile(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),

    #[error("Read-only filesystem")]
    ReadOnly,

    #[error("Encryption error: {0}")]
    EncryptionError(String),

    #[error("Decryption failed: not authorized")]
    DecryptionNotAuthorized,

    #[error("Storage error: {0}")]
    StorageError(#[from] StorageError),

    #[error("Mount point in use: {0}")]
    MountPointInUse(String),
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Network error: {0}")]
    Network(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied")]
    PermissionDenied,

    #[error("Quota exceeded")]
    QuotaExceeded,
}
```

---

## ManifestEntry (확장)

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ManifestEntry {
    /// 파일 경로 (상대 경로)
    pub path: String,

    /// 제목/설명
    pub title: String,

    /// 파일 크기 (바이트)
    #[serde(default)]
    pub size: Option<u64>,

    /// 생성 시간 (Unix timestamp)
    #[serde(default)]
    pub created: Option<u64>,

    /// 수정 시간 (Unix timestamp)
    #[serde(default)]
    pub modified: Option<u64>,

    /// 암호화 여부
    #[serde(default)]
    pub encrypted: bool,

    /// 암호화 정보 (encrypted=true일 때)
    #[serde(default)]
    pub encryption: Option<EncryptionInfo>,
}
```

### manifest.json 예시

```json
[
  {
    "path": "blog/hello.md",
    "title": "Hello World",
    "size": 1234,
    "created": 1704067200,
    "modified": 1704153600,
    "encrypted": false
  },
  {
    "path": "secrets/wallet-backup.enc",
    "title": "Wallet Backup",
    "size": 2048,
    "created": 1704067200,
    "modified": 1704067200,
    "encrypted": true,
    "encryption": {
      "algorithm": "AES-256-GCM",
      "wrapped_keys": [
        {
          "recipient": "0x1234...abcd",
          "encrypted_symmetric_key": "base64encodedkey..."
        }
      ]
    }
  }
]
```

---

## 쓰기 플로우

### 파일 생성/수정

```
┌─────────────────────────────────────────────────────────┐
│ 1. create_file("/home/wonjae/new.md", content)          │
│                         │                               │
│ 2. get_backend_for_path("/home/wonjae/new.md")          │
│    → "/" 매칭 → GitHubBackend                           │
│                         │                               │
│ 3. GitHubBackend.write("home/wonjae/new.md", content)   │
│    → GitHub API 호출                                    │
│    → 새 파일 생성/커밋                                  │
│                         │                               │
│ 4. 성공 시 로컬 FsEntry 트리 업데이트                   │
│                         │                               │
│ 5. manifest.json 업데이트 (선택적)                      │
└─────────────────────────────────────────────────────────┘
```

### 암호화된 파일 생성

```
┌─────────────────────────────────────────────────────────┐
│ 1. encrypt("/home/wonjae/secret.md", ["0x1234.."])      │
│                         │                               │
│ 2. 랜덤 AES-256 키 생성                                 │
│                         │                               │
│ 3. AES-256-GCM으로 파일 내용 암호화                     │
│                         │                               │
│ 4. 수신자(0x1234..)의 공개키로 AES 키 래핑              │
│                         │                               │
│ 5. 암호화된 내용 + EncryptionInfo 저장                  │
│                         │                               │
│ 6. FileMetadata.encrypted = true 설정                   │
└─────────────────────────────────────────────────────────┘
```

---

## 권한 표시 (drwx)

Unix 스타일 권한 표기를 사용하되, 실제 권한은 저장하지 않고 런타임에 계산.

### 권한 계산 규칙

| 권한 | 조건 |
|------|------|
| **d** | 디렉토리이면 `d`, 파일이면 `-` |
| **r** | 암호화 안됨 → 항상 `r` <br> 암호화됨 → 내 지갑 주소가 `wrapped_keys[].recipient`에 있으면 `r` |
| **w** | admin 로그인 → `w` <br> permissionless 마운트 → `w` <br> 그 외 → `-` |
| **x** | 디렉토리 → `x` <br> 파일 → `-` (향후 확장 가능) |

### MountInfo 확장

```rust
pub struct MountInfo {
    pub backend: Box<dyn StorageBackend>,
    pub readonly: bool,
    pub permissionless_write: bool,  // true면 누구나 쓰기 가능
    pub mounted_at: u64,
}
```

### 구현

```rust
pub struct DisplayPermissions {
    pub is_dir: bool,
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl DisplayPermissions {
    pub fn to_string(&self) -> String {
        format!(
            "{}{}{}{}",
            if self.is_dir { 'd' } else { '-' },
            if self.read { 'r' } else { '-' },
            if self.write { 'w' } else { '-' },
            if self.execute { 'x' } else { '-' },
        )
    }
}

impl VirtualFs {
    pub fn get_permissions(
        &self,
        path: &str,
        entry: &FsEntry,
        wallet: &WalletState,
    ) -> DisplayPermissions {
        let is_dir = matches!(entry, FsEntry::Directory { .. });

        // r: 읽기 권한
        let read = match entry {
            FsEntry::Directory { .. } => true,
            FsEntry::File { meta, .. } => {
                if !meta.encrypted {
                    true
                } else if let Some(ref enc) = meta.encryption {
                    wallet.address.as_ref().map_or(false, |addr| {
                        enc.wrapped_keys.iter().any(|k| &k.recipient == addr)
                    })
                } else {
                    false
                }
            }
        };

        // w: 쓰기 권한
        let write = if wallet.is_admin {
            true
        } else if let Some(mount) = self.get_mount_for_path(path) {
            mount.permissionless_write
        } else {
            false
        };

        // x: 실행 권한
        let execute = is_dir;

        DisplayPermissions { is_dir, read, write, execute }
    }
}
```

### UI 표시 예시

```
┌─────────────────────────────────────────┐
│  drwx  blog/                          > │  디렉토리
│  drwx  projects/                      > │  디렉토리
│  -rw-  about.md                    1.2K │  admin 로그인 상태
│  -r--  public.md                   2.1K │  guest (읽기만)
│  ----  secret.enc                  0.8K │  암호화됨, 권한 없음
│  -r--  shared.enc              🔒  1.5K │  암호화됨, 내가 recipient
└─────────────────────────────────────────┘
```

### x 권한 확장 가능성 (향후)

| 파일 타입 | x 의미 |
|-----------|--------|
| `.wasm` | 실행 가능 |
| `.link` | 열기 가능 |
| 인터랙티브 `.md` | 코드 블록 실행 가능 |

---

## 제외 항목

다음 Unix 파일시스템 기능은 이 프로젝트에서 불필요하여 제외:

| 기능 | 제외 이유 |
|------|-----------|
| 권한 저장 | 런타임 계산으로 대체 (암호화 + 로그인 상태 기반) |
| uid/gid | 지갑 주소로 대체 |
| 심볼릭 링크 | 복잡도 대비 효용 낮음 |
| 하드링크 | 불필요 |
| 특수 파일 (블록/캐릭터/소켓) | 웹 환경에서 의미 없음 |
| 파일 디스크립터 테이블 | 파일이 작아서 전체 로드 OK |
| flock (파일 잠금) | 단일 사용자 편집 가정 |

---

## 구현 우선순위

### Phase 1: 메타데이터 (Explorer UI 지원)
- [ ] `FileMetadata` 구조체 추가
- [ ] `ManifestEntry` 확장
- [ ] `from_manifest()` 수정
- [ ] `list_dir()` 메타데이터 반환

### Phase 2: 쓰기 연산 (Admin 기능)
- [ ] `StorageBackend` trait 정의
- [ ] `GitHubBackend` 구현
- [ ] 쓰기 연산 (`create_file`, `write_file`, `delete`, `rename`, `mkdir`)
- [ ] 에러 타입 세분화

### Phase 3: 암호화 (접근 제어)
- [ ] `EncryptionInfo` 구조체
- [ ] `encrypt()` / `decrypt()` 구현
- [ ] `grant_access()` / `revoke_access()` 구현

### Phase 4: 마운트 (다중 저장소)
- [ ] `MountInfo` 구조체
- [ ] `mount()` / `unmount()` 구현
- [ ] `IpfsBackend` 구현
- [ ] `IndexedDbBackend` 구현
