# devhub API

`http://127.0.0.1:7878` で待ち受け。PC 内からのみ接続できます。

- 返り値はすべて `{"ok": true, "data": ...}` か `{"ok": false, "error": "..."}`
- **GET 以外は `X-Devhub: 1` ヘッダーが必須**（Web ページからの不正操作防止）
- 距離の単位は **0.001mm**（`1500` = 1.5mm）
- 書き込むとデバイス本体に保存されます（アプリを閉じても残る）

## 状態

| メソッド | パス | 内容 |
|---|---|---|
| GET | `/api/status` | 接続状況 |
| GET | `/api/keycodes` | キーコード一覧（`name` / `label`） |

## キーボード（ZENAIM）

### GET `/api/keyboard?profile=0`

`profile` を省くと使用中のプロファイル。0=ベース 1=一時 2〜4=MEM1〜3。

各キー: `index` `label` `pointOn` `pointOff` `rapidTrigger` `rt{range,strokeOn,strokeOff,beyond}` `keycodes[通常,無効化,Fn]`

### PATCH `/api/keyboard`

変えたい項目だけ送る。

```json
{
  "profile": 0,
  "all":  { "pointOn": 1200 },
  "keys": [
    { "key": "A", "rapidTrigger": true, "rt": { "strokeOn": 150, "strokeOff": 150 } },
    { "key": "D", "pointOn": 1000, "pointOff": 800 },
    { "key": "Caps", "keycode": "KC_LCTL" }
  ],
  "socd": [ { "mode": 3, "key1": "A", "key2": "D" }, { "mode": 0 } ],
  "holdTime": { "on": 0, "off": 0 },
  "activeProfile": 0
}
```

| 項目 | 範囲 | 意味 |
|---|---|---|
| `pointOn` | 50〜1800 | ON になる深さ |
| `pointOff` | 0〜pointOn未満 | OFF に戻る深さ（浅いほど早く離れる） |
| `rapidTrigger` | true/false | ラピッドトリガー |
| `rt.strokeOn` / `rt.strokeOff` | 1〜1800 | RT の押し込み / 戻り感度 |
| `rt.range` | 0〜1800 | RT が働く範囲（スタビライザー付きキーは自動補正） |
| `rt.beyond` | true/false | ビヨンドモード |
| `keycode` / `fnKeycode` / `disableKeycode` | `"A"` `"KC_A"` `"0x04"` | キー割り当て |
| `socd[].mode` | 0〜5 | OFF / ニュートラル / 先押し優先 / 後押し優先 / キー1優先 / キー2優先 |
| `holdTime.on` / `off` | 0〜1000 | 判定確定までの保持時間（チャタリング対策・全キー共通） |

`key` は今の通常レイヤーの割り当て名で探します。`index` で直接指定も可。

### GET `/api/keyboard/travel`

全キーの現在の押し込み量（`depth`, 0.001mm）と ON/OFF。誤入力の診断用。

## マウス（LAMZU Maya X）

### GET `/api/mouse?profile=1`

`profile` は **1〜3**（省略で使用中）。マウスがスリープ中だとエラー。動かしてから再実行。

### PATCH `/api/mouse`

```json
{
  "profile": 1,
  "pollingRate": 4000,
  "debounceMs": 4,
  "dpiStages": [ { "x": 800, "y": 800 }, { "x": 1600, "y": 1600 } ],
  "activeStage": 1,
  "lod": 1,
  "motionSync": true,
  "rippleControl": false,
  "angleSnap": false,
  "competitionMode": true,
  "dpiXy": false,
  "angleTune": 0,
  "sleepSec": 60,
  "activeProfile": 1
}
```

| 項目 | 値 |
|---|---|
| `pollingRate` | 有線/1K: 125〜1000、8K ドングル: 500/1000/2000/4000/8000 |
| `debounceMs` | 0〜15（クリックのチャタリング対策） |
| `dpiStages` | 1〜5 段、50〜30000 の 50 刻み |
| `activeStage` | 1 始まり |
| `lod` | 0.7 / 1 / 2 |
| `angleTune` | -30〜30 |
| `sleepSec` | 10〜1800 |

## ゲームモード

ゲームが前面にある間だけ、キーボード（とマウス）のプロファイルを自動で切り替える。
ゲーム用プロファイルは `baseProfile` のコピーで、`disableKeys` のキーだけ「なし」になる。
ベースを `PATCH /api/keyboard` で変えると自動で作り直される。切り替えは保存しない（再起動で元に戻る）。

| メソッド | パス | 内容 |
|---|---|---|
| GET | `/api/games` | ルール一覧と現在の状態（`state.active` = 有効中のゲーム） |
| PUT | `/api/games` | ルールを丸ごと保存し、ゲーム用プロファイルを作り直す |
| POST | `/api/games/sync` | ゲーム用プロファイルを作り直す |

```json
[
  {
    "name": "Apex Legends",
    "enabled": true,
    "exe": ["r5apex.exe", "r5apex_dx12.exe"],
    "keyboardProfile": 4,
    "baseProfile": 0,
    "disableKeys": ["Win"],
    "mouseProfile": null
  }
]
```

保存先: `%APPDATA%\devhub\games.json`

## プリセット

| メソッド | パス | 内容 |
|---|---|---|
| GET | `/api/presets` | 一覧 |
| POST | `/api/presets` `{"name":"VALORANT"}` | 今の設定を保存 |
| POST | `/api/presets/apply` `{"name":"VALORANT"}` | 適用 |
| DELETE | `/api/presets?name=VALORANT` | 削除 |

保存先: `%APPDATA%\devhub\presets\`

## 例（PowerShell から）

```powershell
curl.exe -s -X PATCH -H "X-Devhub: 1" -d '{\"keys\":[{\"key\":\"A\",\"pointOff\":1300}]}' http://127.0.0.1:7878/api/keyboard
```
