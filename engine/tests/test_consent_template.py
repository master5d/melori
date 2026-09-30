import pytest

from melori_engine.consent_template import CURRENT_TEMPLATE_VERSION, render_consent
from melori_engine.practice.models import Client, Consent


def client(perms, retain_days=None):
    return Client(id="anna", alias="Анна", tags=[], created="2026-09-27",
                  consent=Consent(permissions=frozenset(perms), date="2026-09-27",
                                  retain_days=retain_days))


def test_lists_granted_and_denied_permissions():
    text = render_consent(client({"transcript", "retain"}, retain_days=90))
    assert "[x] Транскрипт" in text and "[ ] Видео" in text and "[ ] Разбор" in text
    assert "90 дней" in text and "Анна" in text


def test_no_retain_says_nothing_is_kept():
    retention = render_consent(client({"transcript"})).split("Хранение после сессии:")[1].splitlines()[0]
    assert "не сохраняется" in retention


def test_retain_until_revoked():
    assert "до отзыва" in render_consent(client({"transcript", "retain"}, retain_days=None))


def test_unknown_version_raises():
    with pytest.raises(KeyError):
        render_consent(client({"transcript"}), version="v999")


def test_current_version_is_v1():
    assert CURRENT_TEMPLATE_VERSION == "v1"
