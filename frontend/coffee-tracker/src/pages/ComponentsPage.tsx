import { Coffee, Trophy } from 'lucide-react'
import { Button } from '../components/Button'
import { Card, Skeleton } from '../components/Card'
import { LogoMark } from '../components/LogoMark'
import { PageHeader } from '../components/PageHeader'
import achievementsFixture from '../../fixtures/achievements.json'
import leaderboardFixture from '../../fixtures/leaderboard.json'
import beveragesFixture from '../../fixtures/beverages.json'

export function ComponentsPage() {
  return (
    <div>
      <PageHeader eyebrow="Gallery" title="Component states" />
      <div className="grid gap-5 xl:grid-cols-2">
        <Card>
          <p className="mb-4 font-display text-xl font-black">Brand</p>
          <div className="flex flex-wrap items-center gap-5">
            <LogoMark />
            <LogoMark label={false} />
          </div>
        </Card>
        <Card>
          <p className="mb-4 font-display text-xl font-black">Buttons</p>
          <div className="flex flex-wrap gap-3">
            <Button>Primary</Button>
            <Button variant="secondary">Secondary</Button>
            <Button variant="ghost">Ghost</Button>
            <Button variant="danger">Danger</Button>
          </div>
        </Card>
        <Card>
          <p className="mb-4 font-display text-xl font-black">Loading</p>
          <div className="grid gap-3">
            <Skeleton className="h-24" />
            <Skeleton className="h-14" />
          </div>
        </Card>
        <Card>
          <p className="mb-4 font-display text-xl font-black">Empty</p>
          <div className="rounded-4xl bg-berry-100 p-6 text-center dark:bg-white/10">
            <Coffee className="mx-auto mb-3 size-10 text-primary dark:text-blush" />
            <p className="font-display text-xl font-black">No cups yet</p>
            <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
              The first tap gets the party started.
            </p>
          </div>
        </Card>
        <Card>
          <p className="mb-4 font-display text-xl font-black">Beverage cards</p>
          <div className="grid gap-3 sm:grid-cols-2">
            {beveragesFixture.beverages.map((beverage) => (
              <div
                key={beverage.id}
                className="rounded-4xl bg-berry-950 p-5 text-white shadow-button even:bg-mint even:text-berry-950"
              >
                <p className="font-display text-2xl font-black">{beverage.name}</p>
                <p className="font-bold">{beverage.price_cents / 100} €</p>
              </div>
            ))}
          </div>
        </Card>
        <Card>
          <p className="mb-4 font-display text-xl font-black">Achievements</p>
          <div className="grid gap-3 sm:grid-cols-2">
            {achievementsFixture.achievements.slice(0, 2).map((achievement) => (
              <div
                key={achievement.id}
                className="rounded-4xl bg-berry-100 p-5 shadow-soft dark:bg-white/10"
              >
                <Trophy className="mb-3 size-7 text-sun" />
                <p className="font-display font-black">{achievement.name}</p>
                <p className="text-sm font-bold text-berry-700 dark:text-rose-100">
                  {achievement.description ?? achievement.hint}
                </p>
              </div>
            ))}
          </div>
        </Card>
        <Card className="xl:col-span-2">
          <p className="mb-4 font-display text-xl font-black">Leaderboard populated</p>
          <div className="grid gap-3 sm:grid-cols-3">
            {leaderboardFixture.entries.map((entry) => (
              <div key={entry.user_id} className="rounded-4xl bg-sun p-5 text-berry-950">
                <p className="font-display text-xl font-black">#{entry.rank}</p>
                <p className="font-black">{entry.display_name}</p>
                <p className="text-sm font-bold">{entry.coffee_count} coffees</p>
              </div>
            ))}
          </div>
        </Card>
      </div>
    </div>
  )
}
