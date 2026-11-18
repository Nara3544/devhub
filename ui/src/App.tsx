import { useCallback, useEffect, useState } from 'react'
import { TabList, Tab } from '@astryxdesign/core/TabList'
import { Heading } from '@astryxdesign/core/Heading'
import { Text } from '@astryxdesign/core/Text'
import { VStack } from '@astryxdesign/core/VStack'
import { HStack } from '@astryxdesign/core/HStack'
import { Card } from '@astryxdesign/core/Card'
import { Button } from '@astryxdesign/core/Button'
import { Switch } from '@astryxdesign/core/Switch'
import { TextInput } from '@astryxdesign/core/TextInput'
import { StatusDot } from '@astryxdesign/core/StatusDot'
import { Banner } from '@astryxdesign/core/Banner'
import { api, type Status } from './api'
import { KeyboardPanel } from './Keyboard'
import { MousePanel } from './Mouse'
import { GamesPanel } from './Games'

type Toast = { msg: string; err: boolean; id: number }

export function App() {
  const [tab, setTab] = useState('keyboard')
  const [status, setStatus] = useState<Status | null>(null)
  const [toast, setToast] = useState<Toast | null>(null)

  const refresh = useCallback(() => { api.get<Status>('/api/status').then(setStatus).catch(() => setStatus(null)) }, [])
  useEffect(() => { refresh() }, [refresh])

  const onToast = useCallback((msg: string, err = false) => {
    const id = Date.now()
    setToast({ msg, err, id })
    setTimeout(() => setToast(t => (t?.id === id ? null : t)), err ? 6000 : 2500)
  }, [])

  return (
    <div className="shell">
      <div className="header">
        <Heading level={1}>devhub</Heading>
        <HStack gap={4} vAlign="center">
          <StatusDot variant={status?.keyboard.connected ? 'success' : 'neutral'} label={status?.keyboard.connected ? 'キーボード接続中' : 'キーボード未接続'} />
          <Text size="sm">キーボード</Text>
          <StatusDot variant={status?.mouse.connected ? 'success' : 'neutral'} label={status?.mouse.connected ? 'マウス接続中' : 'マウス未接続'} />
          <Text size="sm">マウス</Text>
          <Button label="更新" size="sm" variant="ghost" onClick={refresh} />
        </HStack>
      </div>
      <TabList value={tab} onChange={setTab} hasDivider>
        <Tab value="keyboard" label="キーボード" />
        <Tab value="mouse" label="マウス" />
        <Tab value="games" label="ゲーム" />
        <Tab value="presets" label="プリセット" />
        <Tab value="settings" label="設定" />
      </TabList>
      <div style={{ marginTop: 16 }}>
        {tab === 'keyboard' && <KeyboardPanel onToast={onToast} />}
        {tab === 'mouse' && <MousePanel onToast={onToast} />}
        {tab === 'games' && <GamesPanel onToast={onToast} />}
        {tab === 'presets' && <Presets onToast={onToast} />}
        {tab === 'settings' && <Settings version={status?.version} onToast={onToast} />}
      </div>
      {toast && (
        <div style={{ position: 'fixed', right: 20, bottom: 20, maxWidth: 420, zIndex: 10 }}>
          <Banner status={toast.err ? 'error' : 'success'} title={toast.msg} isDismissable onDismiss={() => setToast(null)} />
        </div>
      )}
    </div>
  )
}

function Presets({ onToast }: { onToast: (m: string, e?: boolean) => void }) {
  const [list, setList] = useState<string[]>([])
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const load = useCallback(() => { api.get<string[]>('/api/presets').then(setList).catch(() => {}) }, [])
  useEffect(load, [load])
  const run = async (f: () => Promise<unknown>, msg: string) => {
    setBusy(true)
    try { await f(); onToast(msg); load() } catch (e) { onToast((e as Error).message, true) } finally { setBusy(false) }
  }
  return (
    <VStack gap={4}>
      <Card padding={4}>
        <VStack gap={3}>
          <Heading level={3}>今の設定を保存</Heading>
          <Text color="secondary" size="sm">キーボード（使用中のプロファイル）とマウスの設定をまとめて保存</Text>
          <HStack gap={2} vAlign="end">
            <TextInput label="名前" value={name} onChange={setName} placeholder="例: VALORANT" width={240} />
            <Button label="保存" variant="primary" isDisabled={!name.trim()} isLoading={busy}
              onClick={() => run(() => api.post('/api/presets', { name: name.trim() }), '保存しました').then(() => setName(''))} />
          </HStack>
        </VStack>
      </Card>
      <Card padding={4}>
        <VStack gap={2}>
          <Heading level={3}>保存済み</Heading>
          {list.length === 0 && <Text color="secondary">まだありません</Text>}
          {list.map(n => (
            <HStack key={n} gap={2} vAlign="center" justify="between">
              <Text>{n}</Text>
              <HStack gap={2}>
                <Button label="適用" size="sm" variant="secondary" isDisabled={busy}
                  onClick={() => run(() => api.post('/api/presets/apply', { name: n }), `${n} を適用しました`)} />
                <Button label="削除" size="sm" variant="ghost" isDisabled={busy}
                  onClick={() => run(() => api.del(`/api/presets?name=${encodeURIComponent(n)}`), '削除しました')} />
              </HStack>
            </HStack>
          ))}
        </VStack>
      </Card>
    </VStack>
  )
}

function Settings({ version, onToast }: { version?: string; onToast: (m: string, e?: boolean) => void }) {
  const [auto, setAuto] = useState<boolean | null>(null)
  useEffect(() => { api.get<boolean>('/api/autostart').then(setAuto).catch(() => {}) }, [])
  return (
    <VStack gap={4}>
      <Card padding={4}>
        <VStack gap={3}>
          <Switch label="Windows 起動時にバックグラウンドで起動" value={!!auto} isDisabled={auto === null}
            onChange={v => api.put<boolean>('/api/autostart', v).then(x => { setAuto(x); onToast(x ? '登録しました' : '解除しました') }).catch(e => onToast(e.message, true))} />
          <Text color="secondary" size="sm">設定はデバイス本体に保存されるので、常駐しなくても反映は保たれます</Text>
        </VStack>
      </Card>
      <Card padding={4}>
        <VStack gap={2}>
          <Heading level={3}>API</Heading>
          <Text color="secondary" size="sm">Claude などから http://127.0.0.1:7878/api/ で操作できます（書き込みは X-Devhub ヘッダー必須）</Text>
          <Text size="sm" type="code">curl -X PATCH -H "X-Devhub: 1" -d '{"{"}"keys":[{"{"}"key":"A","rapidTrigger":true{"}"}]{"}"}' http://127.0.0.1:7878/api/keyboard</Text>
        </VStack>
      </Card>
      <Text color="secondary" size="sm">devhub {version ?? ''}</Text>
    </VStack>
  )
}
