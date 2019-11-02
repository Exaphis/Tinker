import asyncio
import flask

from . import secrets

loop = asyncio.get_event_loop()

app = flask.Flask(__name__)
app.secret_key = secrets.SECRET_KEY
