// Made-up values for the tests of the packages that draw an actor: the view of an actor as the app layer sends it. The
// tone is given, never worked out: the rule is the app layer's (`ActorView::of`), and a test says which tone it wants.
import type { ActorTone, ActorView } from '@meldframe/protocol';

export const actorView = (actor: string, tone: ActorTone = actor.startsWith('agent:') ? 'agent1' : 'person', name = actor.replace(/^[a-z]+:/, '')): ActorView => ({
  actor, kind: actor.startsWith('agent:') ? 'agent' : 'person', name, tone,
});
