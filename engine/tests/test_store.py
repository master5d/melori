from datetime import date
from pathlib import Path

import pytest

from melori_engine.practice import store as st
from melori_engine.practice.models import Consent

ALL = frozenset({"transcript", "video", "council", "retain"})


def mk(root: Path, alias="Анна", perms=ALL, retain_days=None):
    return st.create_client(root, alias, tags=["t"], consent=Consent(
        permissions=frozenset(perms), date="2026-09-27", retain_days=retain_days))


def test_cyrillic_alias_translit(tmp_path):
    assert mk(tmp_path).id == "anna"


def test_emoji_alias_gets_hash_id(tmp_path):
    assert mk(tmp_path, alias="🙂🙂").id.startswith("client-")


@pytest.mark.parametrize("bad", ["../x", "A", "", "a/b", ".."])
def test_unsafe_client_id_rejected(tmp_path, bad):
    with pytest.raises(st.StoreError):
        st.get_client(tmp_path, bad)


def test_unknown_permission_rejected(tmp_path):
    with pytest.raises(st.StoreError):
        mk(tmp_path, perms={"transcript", "telepathy"})


@pytest.mark.parametrize("perm", ["transcript", "video", "council", "retain"])
def test_each_permission_denied_without_it_and_allowed_with_it(tmp_path, perm):
    c_without = mk(tmp_path, alias="bez", perms=ALL - {perm})
    with pytest.raises(st.ConsentError):
        st.require_permission(tmp_path, c_without.id, perm)
    c_with = mk(tmp_path, alias="s", perms=ALL)
    assert st.require_permission(tmp_path, c_with.id, perm).id == c_with.id


def test_revoked_blocks_everything(tmp_path):
    c = mk(tmp_path)
    st.revoke(tmp_path, c.id, today="2026-09-28")
    for perm in ALL:
        with pytest.raises(st.ConsentError):
            st.require_permission(tmp_path, c.id, perm)


def test_open_session_requires_transcript(tmp_path):
    c = mk(tmp_path, perms=ALL - {"transcript"})
    with pytest.raises(st.ConsentError):
        st.open_session(tmp_path, c.id, date="2026-09-27", meeting_type="session")


def test_close_without_retain_leaves_nothing(tmp_path):
    c = mk(tmp_path, perms={"transcript"})
    s = st.open_session(tmp_path, c.id, date="2026-09-27", meeting_type="session")
    saved, retained = st.close_session(tmp_path, c.id, s.id, note="TEXT-MARK")
    assert saved is None and retained is False
    assert not any("TEXT-MARK" in p.read_text(encoding="utf-8") for p in tmp_path.rglob("*.md"))
    assert st.list_sessions(tmp_path, c.id) == []


def test_close_with_retain_persists(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-27", meeting_type="session")
    saved, retained = st.close_session(tmp_path, c.id, s.id, note="hello")
    assert retained is True and saved.status == "closed" and saved.note == "hello"


def test_revoke_mid_session_blocks_close(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-27", meeting_type="session")
    st.revoke(tmp_path, c.id, today="2026-09-27")
    with pytest.raises(st.ConsentError):
        st.close_session(tmp_path, c.id, s.id, note="x")


def test_append_run_requires_council(tmp_path):
    c = mk(tmp_path, perms=ALL - {"council"})
    s = st.open_session(tmp_path, c.id, date="2026-09-27", meeting_type="session")
    with pytest.raises(st.ConsentError):
        st.append_run(tmp_path, c.id, s.id, mode="practitioner", specialist_ids=[], result={})


def test_delete_wipes_every_file(tmp_path):
    c = mk(tmp_path)
    s = st.open_session(tmp_path, c.id, date="2026-09-27", meeting_type="session")
    st.close_session(tmp_path, c.id, s.id, note="x")
    st.delete_client(tmp_path, c.id)
    assert not (tmp_path / c.id).exists()
    assert st.get_client(tmp_path, c.id) is None


def test_purge_expired_boundary(tmp_path):
    c = mk(tmp_path, retain_days=30)
    s1 = st.open_session(tmp_path, c.id, date="2026-08-01", meeting_type="session")
    st.close_session(tmp_path, c.id, s1.id, note="old")
    s2 = st.open_session(tmp_path, c.id, date="2026-08-28", meeting_type="session")
    st.close_session(tmp_path, c.id, s2.id, note="edge")
    open_old = st.open_session(tmp_path, c.id, date="2026-07-01", meeting_type="session")
    deleted = st.purge_expired(tmp_path, today=date(2026, 9, 27))
    assert deleted == [f"{c.id}/{s1.id}"]
    remaining = {s.id for s in st.list_sessions(tmp_path, c.id)}
    assert s2.id in remaining
    assert open_old.id in remaining


def test_retain_until_revoked_never_purged(tmp_path):
    c = mk(tmp_path, retain_days=None)
    s = st.open_session(tmp_path, c.id, date="2020-01-01", meeting_type="session")
    st.close_session(tmp_path, c.id, s.id, note="x")
    assert st.purge_expired(tmp_path, today=date(2026, 9, 27)) == []


def test_attach_signed_consent_records_hash(tmp_path):
    c = mk(tmp_path)
    got = st.attach_signed_consent(tmp_path, c.id, data=b"signed", template_version="v1")
    import hashlib
    assert got.consent.signed_sha256 == hashlib.sha256(b"signed").hexdigest()
    assert got.consent.template_version == "v1"
