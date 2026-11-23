package invites

import (
	"sort"
	"time"

	"github.com/TheArctesian/anima/link/tunnel"
)

// Life is how long an invite can be used (LNK-003, as MET-012).
const Life = 15 * time.Minute

// Refusal is a rule of the invites that a request broke.
type Refusal struct{ code, text string }

func (r *Refusal) Error() string { return r.text }

// Code names the rule for a program.
func (r *Refusal) Code() string { return r.code }

// The rules of the invites.
var (
	ErrBadInvite = &Refusal{"bad_invite", "the text is not an invite"}
	ErrNotOpen   = &Refusal{"invite_not_open", "no open invite has this key: it is used, expired, cancelled or wrong"}
)

// Record is what the hub keeps of one invite. It holds no secret: Key is the public half of the temporary key.
type Record struct {
	Key tunnel.DeviceKey `json:"key"`
	// Name is the name that the new device gets.
	Name    string    `json:"name"`
	Expires time.Time `json:"expires"`
}

// book is the set of open invites, by temporary key.
type book map[tunnel.DeviceKey]Record

func (b book) open(key tunnel.DeviceKey, now time.Time) bool {
	record, known := b[key]
	return known && now.Before(record.Expires)
}

func (b book) without(key tunnel.DeviceKey, now time.Time) (book, Record, error) {
	if !b.open(key, now) {
		return nil, Record{}, ErrNotOpen
	}
	return b.keeping(func(each Record) bool { return each.Key != key }), b[key], nil
}

func (b book) with(record Record, now time.Time) book {
	next := b.keeping(func(each Record) bool { return now.Before(each.Expires) })
	next[record.Key] = record
	return next
}

func (b book) keeping(keep func(Record) bool) book {
	next := book{}
	for key, record := range b {
		if keep(record) {
			next[key] = record
		}
	}
	return next
}

func (b book) openAt(now time.Time) []Record {
	return b.keeping(func(each Record) bool { return now.Before(each.Expires) }).all()
}

func (b book) all() []Record {
	all := []Record{}
	for _, record := range b {
		all = append(all, record)
	}
	sort.Slice(all, func(i, j int) bool { return all[i].Expires.Before(all[j].Expires) })
	return all
}
