import { useViewMode } from '../lib/viewMode'

export function SimpleViewSwitch() {
  const { simple, setSimple, saving } = useViewMode()
  return (
    <div className="flex items-center justify-between gap-4 border-b border-primary/10 pb-4">
      <span>
        <span id="simple-view-label" className="block font-display font-black">Simple view</span>
        <span id="simple-view-description" className="text-sm text-berry-700 dark:text-rose-100">
          Essential drink controls and navigation in a menu. Saved to your account across devices.
        </span>
      </span>
      <button
        type="button"
        role="switch"
        aria-checked={simple}
        aria-labelledby="simple-view-label"
        aria-describedby="simple-view-description"
        disabled={saving}
        onClick={() => setSimple(!simple)}
        className={`inline-flex h-7 w-12 shrink-0 items-center rounded-full p-1 transition-colors disabled:opacity-55 ${simple ? 'bg-primary dark:bg-blush' : 'bg-berry-700/30 dark:bg-white/25'}`}
      >
        <span className={`size-5 rounded-full bg-white shadow-sm transition-transform ${simple ? 'translate-x-5' : 'translate-x-0'}`} />
      </button>
    </div>
  )
}
